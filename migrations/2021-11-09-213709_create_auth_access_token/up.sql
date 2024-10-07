CREATE TABLE `auth_access_token` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `uuid` CHAR(36) NOT NULL UNIQUE,
  `refresh_token_id` INT NOT NULL,
  `access_token` varchar(255) NOT NULL UNIQUE,
  `expiration_time` TIMESTAMP NOT NULL,
  CONSTRAINT `auth_access_token_fk0` FOREIGN KEY (`refresh_token_id`) REFERENCES `auth_refresh_token`(`id`)
);
