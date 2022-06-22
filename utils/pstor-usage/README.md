# Persistent Storage Usage

This `pstore-usage` can be used to sample persistent store (ETCD) usage at runtime from a simulated cluster as well as extrapolate future usage based on the samples.
By default, it makes use of the `deployer` library to create a local cluster running on docker.

## Examples

**Using the help**

```textmate
❯ cargo run -q --bin pstor-usage -- --help

USAGE:
    pstor-usage [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -r, --rest-url <rest-url>    The rest endpoint if reusing a cluster

SUBCOMMANDS:
    extrapolate    Extrapolate how much storage a cluster would require if it were to run for a specified number of
                   days
    help           Prints this message or the help of the given subcommand(s)
    simulate       Simulate how much storage a cluster would require based on some parameters
```

Help can also be retrieved for a specific subcommand, example:
```textmate
> cargo run -q --bin pstor-usage -- simulate --help
Simulate how much storage a cluster would require based on some parameters

USAGE:
    pstor-usage simulate [FLAGS] [OPTIONS]

FLAGS:
    -h, --help               Prints help information
        --pool-use-malloc    Use ram based pools instead of files (useful for debugging with small pool allocation).
                             When using files the /tmp/pool directory will be used
        --no-total-stats     Skip the output of the total storage usage after allocation of all resources and also after
                             those resources have been deleted
    -V, --version            Prints version information

OPTIONS:
        --pool-samples <pool-samples>                  Number of pool samples [default: 5]
        --pool-size <pool-size>                        Size of the pools [default: 20MiB]
    -p, --pools <pools>                                Number of pools per sample [default: 10]
        --volume-attach-cycles <volume-attach-cycle>
            Attaches and detaches `N` volumes from each volume sample. In other words, we will publish/unpublish each
            `N` volumes from each list of samples. Please note that this can take quite some time; it's very slow to
            create volume targets with remote replicas [default: 2]
        --volume-replicas <volume-replicas>            Number of volume replicas [default: 3]
        --volume-samples <volume-samples>              Number of volume samples [default: 10]
        --volume-size <volume-size>                    Size of the volumes [default: 5MiB]
    -v, --volumes <volumes>                            Number of volumes per sample [default: 20]
```

**Sampling the persistent storage usage**

```textmate
❯ cargo run -q --bin pstor-usage
┌───────────────┬────────────┐
│ Volumes ~Repl │ Disk Usage │
├───────────────┼────────────┤
│     20 ~3     │   92 KiB   │
├───────────────┼────────────┤
│     40 ~3     │  172 KiB   │
├───────────────┼────────────┤
│     60 ~3     │  252 KiB   │
├───────────────┼────────────┤
│     80 ~3     │  332 KiB   │
├───────────────┼────────────┤
│    100 ~3     │  412 KiB   │
├───────────────┼────────────┤
│    120 ~3     │  488 KiB   │
├───────────────┼────────────┤
│    140 ~3     │  580 KiB   │
├───────────────┼────────────┤
│    160 ~3     │  664 KiB   │
├───────────────┼────────────┤
│    180 ~3     │  740 KiB   │
├───────────────┼────────────┤
│    200 ~3     │  824 KiB   │
└───────────────┴────────────┘
┌───────┬────────────┐
│ Pools │ Disk Usage │
├───────┼────────────┤
│  10   │   8 KiB    │
├───────┼────────────┤
│  20   │   16 KiB   │
├───────┼────────────┤
│  30   │   28 KiB   │
├───────┼────────────┤
│  40   │   36 KiB   │
├───────┼────────────┤
│  50   │   44 KiB   │
└───────┴────────────┘
┌───────────────┬───────┬────────────┐
│ Volumes ~Repl │ Pools │ Disk Usage │
├───────────────┼───────┼────────────┤
│     20 ~3     │  10   │  100 KiB   │
├───────────────┼───────┼────────────┤
│     40 ~3     │  20   │  188 KiB   │
├───────────────┼───────┼────────────┤
│     60 ~3     │  30   │  280 KiB   │
├───────────────┼───────┼────────────┤
│     80 ~3     │  40   │  368 KiB   │
├───────────────┼───────┼────────────┤
│    100 ~3     │  50   │  456 KiB   │
├───────────────┼───────┼────────────┤
│    120 ~3     │  50   │  532 KiB   │
├───────────────┼───────┼────────────┤
│    140 ~3     │  50   │  624 KiB   │
├───────────────┼───────┼────────────┤
│    160 ~3     │  50   │  708 KiB   │
├───────────────┼───────┼────────────┤
│    180 ~3     │  50   │  784 KiB   │
├───────────────┼───────┼────────────┤
│    200 ~3     │  50   │  868 KiB   │
└───────────────┴───────┴────────────┘
┌──────────────────┬────────────┐
│ Volume~Repl Mods │ Disk Usage │
├──────────────────┼────────────┤
│       2~3        │   20 KiB   │
├──────────────────┼────────────┤
│       4~3        │   44 KiB   │
├──────────────────┼────────────┤
│       6~3        │   68 KiB   │
├──────────────────┼────────────┤
│       8~3        │   96 KiB   │
├──────────────────┼────────────┤
│       10~3       │  120 KiB   │
├──────────────────┼────────────┤
│       12~3       │  144 KiB   │
├──────────────────┼────────────┤
│       14~3       │  168 KiB   │
├──────────────────┼────────────┤
│       16~3       │  192 KiB   │
├──────────────────┼────────────┤
│       18~3       │  216 KiB   │
├──────────────────┼────────────┤
│       20~3       │  240 KiB   │
└──────────────────┴────────────┘
┌──────────┬──────────────┬─────────┬───────────────┬───────────────┐
│ Creation │ Modification │ Cleanup │     Total     │    Current    │
├──────────┼──────────────┼─────────┼───────────────┼───────────────┤
│ 868 KiB  │ 240 KiB      │ 532 KiB │ 1 MiB 616 KiB │ 1 MiB 636 KiB │
└──────────┴──────────────┴─────────┴───────────────┴───────────────┘
```

***Sampling only single replica volumes:***

```textmate
❯ cargo run -q --bin pstor-usage -- --pools 0 --volume-replicas 1 --volume-attach-cycles 0 --no-total-stats
┌───────────────┬────────────┐
│ Volumes ~Repl │ Disk Usage │
├───────────────┼────────────┤
│     20 ~1     │   40 KiB   │
├───────────────┼────────────┤
│     40 ~1     │   84 KiB   │
├───────────────┼────────────┤
│     60 ~1     │  116 KiB   │
├───────────────┼────────────┤
│     80 ~1     │  160 KiB   │
├───────────────┼────────────┤
│    100 ~1     │  200 KiB   │
├───────────────┼────────────┤
│    120 ~1     │  236 KiB   │
├───────────────┼────────────┤
│    140 ~1     │  276 KiB   │
├───────────────┼────────────┤
│    160 ~1     │  324 KiB   │
├───────────────┼────────────┤
│    180 ~1     │  360 KiB   │
├───────────────┼────────────┤
│    200 ~1     │  408 KiB   │
└───────────────┴────────────┘
```

***Extrapolating persistent storage usage for a whole year:***

```textmate
> Extrapolate how much storage a cluster would require if it were to run for a specified number of days
USAGE:
    pstor-usage extrapolate [FLAGS] [OPTIONS] --days <days> [SUBCOMMAND]

FLAGS:
    -h, --help               Prints help information
        --show-simulation    Show tabulated simulation output
        --usage-bytes        Show the usage in the stdout output as bytes
        --usage-only         Show only the usage in the stdout output
    -V, --version            Prints version information

OPTIONS:
        --cluster-name <cluster-name>
            When using a cluster config (--config), you can specify a single cluster to extrapolate. Otherwise, we'll
            extrapolate all clusters
    -c, --config <config>
            Reads cluster configuration from a YAML configuration file.
            Example format:
            ```yaml
            clusters:
              tiny:
                replicas: 1
                volume_turnover: 1
                volume_attach_cycles: 5
              small:
                replicas: 2
                volume_turnover: 10
                volume_attach_cycles: 15
            ```
            When using this option you must specify which cluster to extrapolate using --cluster-name.
    -d, --days <days>                                  Runtime in days to extrapolate
        --table-entries <table-entries>                Maximum number of table entries to print [default: 10]
        --volume-attach-cycle <volume-attach-cycle>
            Volume attach cycle: how many volume modifications (publish/unpublish) are done every day [default: 200]

        --volume-turnover <volume-turnover>
            Volume turnover: how many volumes are created/deleted every day [default: 50]


SUBCOMMANDS:
    help        Prints this message or the help of the given subcommand(s)
    simulate

❯ cargo run -q --bin pstor-usage -- extrapolate --days 365
┌──────┬─────────────────┬──────────────────────┬─────────────────┐
│ Days │ Volume Turnover │ Volume Attach/Detach │   Disk Usage    │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│  36  │      1800       │         7200         │ 96 MiB 514 KiB  │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│  72  │      3600       │        14400         │  193 MiB 5 KiB  │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 108  │      5400       │        21600         │ 289 MiB 520 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 144  │      7200       │        28800         │ 386 MiB 11 KiB  │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 180  │      9000       │        36000         │ 482 MiB 526 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 216  │      10800      │        43200         │ 579 MiB 17 KiB  │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 252  │      12600      │        50400         │ 675 MiB 532 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 288  │      14400      │        57600         │ 772 MiB 23 KiB  │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 324  │      16200      │        64800         │ 868 MiB 538 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 365  │      18250      │        73000         │ 978 MiB 442 KiB │
└──────┴─────────────────┴──────────────────────┴─────────────────┘
```

The simulation parameters can be modified using the simulation subcommand within the extrapolation, example:

```textmate
❯ cargo run -q --bin pstor-usage -- extrapolate --days 365 simulate --volume-replicas 1
┌──────┬─────────────────┬──────────────────────┬─────────────────┐
│ Days │ Volume Turnover │ Volume Attach/Detach │   Disk Usage    │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│  36  │      1800       │         7200         │ 53 MiB 626 KiB  │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│  72  │      3600       │        14400         │ 107 MiB 228 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 108  │      5400       │        21600         │ 160 MiB 854 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 144  │      7200       │        28800         │ 214 MiB 456 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 180  │      9000       │        36000         │ 268 MiB 59 KiB  │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 216  │      10800      │        43200         │ 321 MiB 685 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 252  │      12600      │        50400         │ 375 MiB 287 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 288  │      14400      │        57600         │ 428 MiB 913 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 324  │      16200      │        64800         │ 482 MiB 516 KiB │
├──────┼─────────────────┼──────────────────────┼─────────────────┤
│ 365  │      18250      │        73000         │ 543 MiB 575 KiB │
└──────┴─────────────────┴──────────────────────┴─────────────────┘
```

***Extrapolating using a config file:***

```textmate
> cargo run -q --bin pstor-usage -- extrapolate --days 365 --config utils/pstor-usage/config.yaml
┌─────────┬───────────────────────────────────────────────────────────────────┐
│ Cluster │                              Results                              │
├─────────┼───────────────────────────────────────────────────────────────────┤
│ tiny    │ ┌──────┬─────────────────┬──────────────────────┬───────────────┐ │
│         │ │ Days │ Volume Turnover │ Volume Attach/Detach │  Disk Usage   │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │  36  │       36        │         180          │ 1 MiB 742 KiB │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │  72  │       72        │         360          │ 3 MiB 461 KiB │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 108  │       108       │         540          │ 5 MiB 180 KiB │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 144  │       144       │         720          │ 6 MiB 923 KiB │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 180  │       180       │         900          │ 8 MiB 642 KiB │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 216  │       216       │         1080         │    11 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 252  │       252       │         1260         │    12 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 288  │       288       │         1440         │    14 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 324  │       324       │         1620         │    16 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 365  │       365       │         1825         │    18 MiB     │ │
│         │ └──────┴─────────────────┴──────────────────────┴───────────────┘ │
├─────────┼───────────────────────────────────────────────────────────────────┤
│ small   │ ┌──────┬─────────────────┬──────────────────────┬───────────────┐ │
│         │ │ Days │ Volume Turnover │ Volume Attach/Detach │  Disk Usage   │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │  36  │       360       │         540          │ 6 MiB 436 KiB │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │  72  │       720       │         1080         │    13 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 108  │      1080       │         1620         │    20 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 144  │      1440       │         2160         │    26 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 180  │      1800       │         2700         │    33 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 216  │      2160       │         3240         │    39 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 252  │      2520       │         3780         │    45 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 288  │      2880       │         4320         │    52 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 324  │      3240       │         4860         │    58 MiB     │ │
│         │ ├──────┼─────────────────┼──────────────────────┼───────────────┤ │
│         │ │ 365  │      3650       │         5475         │    66 MiB     │ │
│         │ └──────┴─────────────────┴──────────────────────┴───────────────┘ │
├─────────┼───────────────────────────────────────────────────────────────────┤
│ medium  │ ┌──────┬─────────────────┬──────────────────────┬────────────┐    │
│         │ │ Days │ Volume Turnover │ Volume Attach/Detach │ Disk Usage │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │  36  │      1800       │         3600         │   55 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │  72  │      3600       │         7200         │  109 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 108  │      5400       │        10800         │  163 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 144  │      7200       │        14400         │  218 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 180  │      9000       │        18000         │  272 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 216  │      10800      │        21600         │  326 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 252  │      12600      │        25200         │  381 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 288  │      14400      │        28800         │  435 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 324  │      16200      │        32400         │  489 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 365  │      18250      │        36500         │  551 MiB   │    │
│         │ └──────┴─────────────────┴──────────────────────┴────────────┘    │
├─────────┼───────────────────────────────────────────────────────────────────┤
│ large   │ ┌──────┬─────────────────┬──────────────────────┬────────────┐    │
│         │ │ Days │ Volume Turnover │ Volume Attach/Detach │ Disk Usage │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │  36  │      3600       │         7200         │  109 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │  72  │      7200       │        14400         │  218 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 108  │      10800      │        21600         │  326 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 144  │      14400      │        28800         │  435 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 180  │      18000      │        36000         │  544 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 216  │      21600      │        43200         │  652 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 252  │      25200      │        50400         │  761 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 288  │      28800      │        57600         │  869 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 324  │      32400      │        64800         │  978 MiB   │    │
│         │ ├──────┼─────────────────┼──────────────────────┼────────────┤    │
│         │ │ 365  │      36500      │        73000         │  1102 MiB  │    │
│         │ └──────┴─────────────────┴──────────────────────┴────────────┘    │
└─────────┴───────────────────────────────────────────────────────────────────┘
```
