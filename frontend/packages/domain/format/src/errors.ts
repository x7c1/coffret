/**
 * Everything that can go wrong writing or reading a coffret object.
 *
 * The codes are the stable part: a caller branches on `code`, never on the
 * message. The header-shape codes are raised before any key is touched, so an
 * object that is not a Container v1 — or not a control object v1 — at all stays
 * distinguishable from one that is but fails to open.
 */
export type CoffretErrorCode =
  // Domain values
  | 'invalid_byte_length'
  | 'invalid_hex_digit'
  | 'invalid_hex_length'
  | 'epoch_out_of_range'
  | 'generation_out_of_range'
  | 'invalid_replica_position'
  | 'invalid_replica_count'
  | 'invalid_set_digest'
  | 'value_out_of_range'
  // Container framing
  | 'header_too_short'
  | 'unknown_magic'
  | 'unsupported_version'
  | 'reserved_not_zero'
  | 'invalid_chunk_size'
  | 'truncated'
  | 'missing_chunks'
  | 'authentication_failed'
  | 'malformed_meta'
  | 'meta_encode_failed'
  | 'unsupported_meta_schema'
  | 'meta_section_too_long'
  | 'empty_entry_table'
  | 'entry_table_not_contiguous'
  | 'stream_too_long'
  | 'plaintext_length_mismatch'
  | 'non_zero_padding'
  | 'non_zero_meta_padding'
  | 'content_hash_mismatch'
  // Control objects
  | 'control_header_too_short'
  | 'unknown_control_magic'
  | 'unsupported_control_version'
  | 'unknown_control_object_kind'
  | 'missing_control_payload'
  | 'wrong_purpose_key'
  | 'malformed_object_name'
  | 'control_object_kind_not_admitted'
  | 'object_name_mismatch'
  | 'malformed_control_payload'
  | 'control_payload_not_a_map'
  | 'non_zero_control_padding'
  | 'control_padding_length_mismatch'
  | 'missing_master_key_epoch'
  | 'control_payload_encode_failed'
  | 'control_payload_too_long'
  // Control-object payload schemas
  | 'malformed_journal_record'
  | 'unsupported_journal_record_schema'
  | 'malformed_index_snapshot'
  | 'unsupported_index_snapshot_schema'
  | 'control_payload_out_of_order'
  | 'snapshot_entry_without_container'
  | 'dangling_container_index'
  | 'activation_field_on_ordinary_snapshot'
  | 'activation_snapshot_field_missing'
  | 'not_an_index_snapshot_kind'
  // Stored Master Key
  | 'unknown_stored_master_key_magic'
  | 'unsupported_stored_master_key_version'
  | 'stored_master_key_length_mismatch'
  | 'invalid_argon2_params'
  | 'passphrase_derivation_failed'
  // Environment
  | 'entropy_unavailable';

/** The one error type this package throws. */
export class CoffretFormatError extends Error {
  /** Which failure this is, for callers to branch on. */
  readonly code: CoffretErrorCode;

  constructor(code: CoffretErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = 'CoffretFormatError';
    this.code = code;
  }
}

/** Raises a [`CoffretFormatError`], for use where an expression is wanted. */
export function fail(code: CoffretErrorCode, message: string, options?: ErrorOptions): never {
  throw new CoffretFormatError(code, message, options);
}
