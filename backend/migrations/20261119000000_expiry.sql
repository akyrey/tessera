-- M6: optional expiry date (coupons, vouchers, yearly memberships), as an ISO date.
ALTER TABLE cards ADD COLUMN expires_on TEXT
    CHECK (expires_on GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]');
