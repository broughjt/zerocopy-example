use bytes::Bytes;
use zerocopy::{U64, BigEndian, LayoutVerified, FromBytes, AsBytes, Unaligned};

// The actual Key type is used to perform a lookup in a database elsewhere in the application logic
#[derive(AsBytes, Clone, Debug, FromBytes, Unaligned)]
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
pub struct Request<K>(pub K);

impl TryFrom<Bytes> for Request<LayoutVerified<Bytes, ExampleKey>> {
    type Error = (); // Actual error type goes here

    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        let length = bytes.len();

        LayoutVerified::new_unaligned(bytes).map(Request).ok_or(())
    }
}
