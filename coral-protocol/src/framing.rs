use crate::limits::*;
use crate::reader::ReadError;

pub fn validate_packet_length(len: i32) -> Result<usize, ReadError> {
    if len <= 0 {
        return Err(ReadError::NegativeLength(len));
    }
    let len = len as usize;
    if len > MAX_COMPRESSED_PACKET_SIZE {
        return Err(ReadError::LengthTooLarge {
            len,
            max: MAX_COMPRESSED_PACKET_SIZE,
        });
    }
    Ok(len)
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompressionEnvelope {
    Uncompressed,
    Compressed { uncompressed_size: usize },
}

pub fn validate_compression_envelope(
    data_length: i32,
    threshold: usize,
) -> Result<CompressionEnvelope, ReadError> {
    if data_length < 0 {
        return Err(ReadError::NegativeLength(data_length));
    }
    let declared = data_length as usize;

    if declared == 0 {
        return Ok(CompressionEnvelope::Uncompressed);
    }
    if declared > MAX_PACKET_SIZE {
        return Err(ReadError::LengthTooLarge {
            len: declared,
            max: MAX_PACKET_SIZE,
        });
    }
    if declared < threshold {
        return Err(ReadError::OutOfBounds);
    }
    Ok(CompressionEnvelope::Compressed {
        uncompressed_size: declared,
    })
}

pub fn validate_uncompressed_size(len: usize, threshold: usize) -> Result<(), ReadError> {
    if len >= threshold {
        return Err(ReadError::OutOfBounds);
    }
    Ok(())
}

pub fn decompress_checked(compressed: &[u8], declared_size: usize) -> Result<Vec<u8>, ReadError> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    if declared_size > MAX_PACKET_SIZE {
        return Err(ReadError::LengthTooLarge {
            len: declared_size,
            max: MAX_PACKET_SIZE,
        });
    }
    let mut out = Vec::with_capacity(declared_size);
    let mut limited = ZlibDecoder::new(compressed).take((declared_size as u64) + 1);
    limited
        .read_to_end(&mut out)
        .map_err(|_| ReadError::OutOfBounds)?;

    if out.len() != declared_size {
        return Err(ReadError::LengthTooLarge {
            len: out.len(),
            max: declared_size,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_and_zero_packet_lengths_rejected() {
        assert!(validate_packet_length(-1).is_err());
        assert!(validate_packet_length(i32::MIN).is_err());
        assert!(validate_packet_length(0).is_err());
    }

    #[test]
    fn oversized_packet_length_rejected() {
        assert!(validate_packet_length(i32::MAX).is_err());
    }

    #[test]
    fn reasonable_length_accepted() {
        assert_eq!(validate_packet_length(1024).unwrap(), 1024);
    }

    #[test]
    fn zero_data_length_means_uncompressed() {
        assert_eq!(
            validate_compression_envelope(0, 256).unwrap(),
            CompressionEnvelope::Uncompressed
        );
    }

    #[test]
    fn compressed_below_threshold_rejected() {
        assert!(validate_compression_envelope(100, 256).is_err());
    }

    #[test]
    fn compressed_above_threshold_accepted() {
        assert_eq!(
            validate_compression_envelope(1000, 256).unwrap(),
            CompressionEnvelope::Compressed {
                uncompressed_size: 1000
            }
        );
    }

    #[test]
    fn negative_or_absurd_data_length_rejected() {
        assert!(validate_compression_envelope(-5, 256).is_err());
        assert!(validate_compression_envelope(i32::MAX, 256).is_err());
    }

    #[test]
    fn uncompressed_above_threshold_rejected() {
        assert!(validate_uncompressed_size(500, 256).is_err());
        assert!(validate_uncompressed_size(100, 256).is_ok());
    }
}
