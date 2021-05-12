CREATE TABLE `album` (
	`id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
	`owner_id` INT NOT NULL,
	`link` varchar(255),
	`password` varchar(255),
  CONSTRAINT `album_fk0` FOREIGN KEY (`owner_id`) REFERENCES `user`(`id`)
);
