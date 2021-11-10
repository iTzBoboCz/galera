CREATE TABLE `auth_refresh_token` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `user_id` INT NOT NULL,
  `refresh_token` varchar(255) NOT NULL UNIQUE,
  `expiration_time` TIMESTAMP NOT NULL,
  CONSTRAINT `auth_refresh_token_fk0` FOREIGN KEY (`user_id`) REFERENCES `user`(`id`)
);
