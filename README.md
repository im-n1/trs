# TranspoRS

Transportation timetambles for command line.

![example](./assets/example.png)

# Installation

```
cargo install --git https://github.com/im-n1/transpors
```

## Usage

Note: When installed the binary is named `trs` for convenience.

### Wizard

When you run the app for the first time a wizard will welcome you and
walk you thru the setup process. All you need is GTFS file URL or location
on your drive.

### Print timetables

Except wiping (`-w`) the app always prints out the timetables of your stations.

```
$ trs

Skloněná -> Sídliště Čakovice
-----------------------------
136 - 20:53
136 - 21:11
136 - 21:31
```

### Refreshing database

When a new version of GTFS file is available you can simply refresh your app database
with just one command. If the GTFL file location is URL it will be downloaded automatically.

```
$ trs -r
```

### Add/delete stops

```
$ trs -a  # to add new stops
$ trs -d  # to delete existing stops
```

### Wiping whole app

```
$ trs -w  # wipes whole app database - cannot be undone.
```

## Changelog

### 0.1

- initial release
