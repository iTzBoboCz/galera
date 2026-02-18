CREATE TABLE auth_session_origin (
  refresh_token_id INT NOT NULL,

  -- 'oidc', 'saml', 'local',..
  method VARCHAR(32) NOT NULL,
  provider_key VARCHAR(128) NULL,

  -- user uuid
  subject VARCHAR(255) NULL,

  -- Method-specific data (JSON string)
  data_json TEXT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY (refresh_token_id),

  CONSTRAINT fk_auth_session_origin_refresh_token
    FOREIGN KEY (refresh_token_id)
    REFERENCES auth_refresh_token(id)
    ON DELETE CASCADE
    ON UPDATE CASCADE,

  INDEX idx_auth_session_origin_method (method),
  INDEX idx_auth_session_origin_provider_key (provider_key)
);
