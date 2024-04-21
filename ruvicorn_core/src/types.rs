use bytes::Bytes;

pub type RsHeader = (Bytes, Bytes);

pub type PyHeader<'t> = (&'t [u8], &'t [u8]);
