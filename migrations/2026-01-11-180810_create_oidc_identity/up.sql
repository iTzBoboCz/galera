ALTER TABLE `user`
  MODIFY `password` VARCHAR(128) NULL;

CREATE TABLE `oidc_identity` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `provider_key` VARCHAR(128) NOT NULL,
  `subject` VARCHAR(255) NOT NULL,
  `user_id` INT NOT NULL,
  `created_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  CONSTRAINT `oidc_identity_fk0`
    FOREIGN KEY (`user_id`) REFERENCES `user`(`id`)
    ON DELETE CASCADE,

  CONSTRAINT `oidc_identity_un0`
    UNIQUE (`provider_key`, `subject`)
);
