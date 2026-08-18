# Rust notes

## Borrowing and ownership

The compiler is not being difficult — it is refusing a program that would fail
at runtime in another language. Fighting the borrow checker is usually a sign
that the design is what is wrong.

## Errors

`Result` instead of exceptions changes how a function is written: the failing
path is visible in the signature, so the caller cannot pretend it is not there.

## Performance

Measure before optimising. Speeding up a function worth 2% of the total changes
nothing while the real bottleneck sits in an allocation inside a loop.
