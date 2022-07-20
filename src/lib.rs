use std::{convert::Infallible, task::{Context, Poll}};

use bytes::Bytes;
use futures_util::future::{Ready, ok};
use zerocopy::{U64, BigEndian, LayoutVerified, FromBytes, AsBytes, Unaligned};
use tower_service::Service;

// The actual Key type is used to perform a lookup in a database elsewhere in the application logic
#[derive(AsBytes, Clone, Debug, Eq, FromBytes, Unaligned, PartialEq)]
#[repr(C)]
pub struct ExampleKey(pub U64<BigEndian>);

// `Request is a wrapper type that implements an internal `FixedLengthDecode`
// trait for parsing the request from bytes from the network. Here I've just
// implemented TryFrom<Bytes> as an example. `Request` has a generic `K` because
// sometimes it is used as a client request and the client might want to pass
// in an owned `Key` or a borrowed reference to one. The client is generic over
// any `K: Borrow<Key>`. On the server side though, I'm using the `quinn`
// library, where requests come in over a `SendStream` as `Bytes` chunks. I want
// to parse the incoming request without copying the underlying bytes. Also, the
// server protocol code is seperated from the application logic using the
// `tower::Service` trait, where the server will use any service
// `S: Service<Request, Response = Response>` to provide responses to the
// client. The problem is that the `Service` trait has no room for explicit
// lifetimes, and the request passed into the `call` method has to be owned.
// This means that I can't pass `LayoutVerified<&'a [u8], Key>` to the
// application code. I tried a hack where I had a wrapper struct that contained
// both the underlying `Bytes` and a `LayoutVerified<&[u8], Key>` which pointed
// to those bytes, but I really struggled to make the compiler happy with that.
// That's why I think I want `Bytes` and `BytesMut` from the `bytes` crate to
// implement `zerocopy::ByteSlice`.
#[derive(Eq, PartialEq)]
pub struct Request<K>(pub K);

// Here is the problem:

// I can add an explicit lifetime here, and the compiler won't complain. The 
// returned request now has a reference to the bytes chunk.
impl<'a> TryFrom<&'a Bytes> for Request<LayoutVerified<&'a [u8], ExampleKey>> {
    type Error = (); // Actual error type goes here

    fn try_from(bytes: &'a Bytes) -> Result<Self, Self::Error> {
        LayoutVerified::new_unaligned(bytes.as_ref()).map(Request).ok_or(())
    }
}

struct ExampleResponse;

struct ExampleService;

// Then we have a service that the server will use to make a response to send 
// back to the client. The compiler will let you elide the explicit lifetime 'a, 
// but I kept it here for clarity.
impl<'a> Service<Request<LayoutVerified<&'a [u8], ExampleKey>>> for ExampleService {
    type Response = ExampleResponse;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: Request<LayoutVerified<&'a [u8], ExampleKey>>) -> Self::Future {
        // use the request
        // produce a response
        ok(ExampleResponse)
    }
}

// Here's where the error shows up:
// The server networking code reads bytes from the connection, parses it, and 
// passes the request to the service to get a response. Unfortunately, because 
// the request has a borrowed reference instead of owning the underlying bytes, 
// the request we pass in doesn't live long enough.
async fn server_networking_code<'a, S>(mut service: S) 
where
    S: Service<Request<LayoutVerified<&'a [u8], ExampleKey>>, Response = ExampleResponse, Error = Infallible>,
{
    const REQUEST: &[u8] = &[0xff; 4];

    // Accept a connection, read bytes
    let incoming_request = Bytes::from(REQUEST);
    // Parse the request by reading bytes from the connection
    let parsed_request = Request::try_from(&incoming_request).unwrap();
    let response = service.call(parsed_request).await.unwrap();
}