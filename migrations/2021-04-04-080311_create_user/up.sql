CREATE TABLE `user` (
  `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
  `username` VARCHAR(60) NOT NULL UNIQUE,
  `email` VARCHAR(254) NOT NULL UNIQUE,
  `password` VARCHAR(128) NOT NULL
);
