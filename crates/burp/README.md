# Burp

## Build

To build the program install `cargo` then execute
```
cargo build --release -p burp
```

The binaries can be found in **traget/**.


## Execute

Move into the **max-cut** directory and execute:
```
cargo run --release -p burp -- [args]
```

For more detailed information about the usage execute:
```
cargo run --release -p -- -h
```

## Test

### Run time

To benchmark the running time of all algorithms run:
```
cargo bench
```
This take a lot of time though.
The results can be found in **target/criterion/**.
For easy viewing just open the **index.html** inside **report/**.
