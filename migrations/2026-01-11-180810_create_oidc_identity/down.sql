ALTER TABLE `user`
  MODIFY `password` VARCHAR(128) NOT NULL;

DROP TABLE oidc_identity
