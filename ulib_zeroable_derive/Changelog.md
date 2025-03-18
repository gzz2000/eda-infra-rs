This is the changelog,summarising changes in each version(some minor changes may be ommited).

### 0.2

- Changed `Zeroable` to require union fields to be `Zeroable` by default.

- Added `#[zero(nonzero)]` and `#[zero(nonzero_fields)]` attributes to opt out 
of requiring Zeroable for a union field.

- Improved error messages when a field's type doesn't implement Zeroable.

- Made the `zeroable::assert_zeroable` module public.

- Added `GetAssertZeroable` inside `zeroable::assert_zeroable`,
used for both constructing a AssertZeroable,
and for improving the "doesn't implement Zeroable" error messages.

- Some improvements to the `Zeroable` docs.

### 0.1

- Defined the `Zeroable` derive macro.

- Defined the `AssertZeroable` type.

