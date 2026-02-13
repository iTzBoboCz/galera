-- 1) add nullable first so existing rows don't violate NOT NULL
ALTER TABLE `user`
  ADD COLUMN `uuid` VARCHAR(36) NULL;

-- 2) backfill existing users
UPDATE `user`
SET `uuid` = UUID()
WHERE `uuid` IS NULL;

-- 3) enforce constraints
ALTER TABLE `user`
  MODIFY `uuid` VARCHAR(36) NOT NULL;

ALTER TABLE `user`
  ADD UNIQUE KEY `user_uuid_unique` (`uuid`);
