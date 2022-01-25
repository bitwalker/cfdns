# cfdns

This utility is what I run on my Ubiquiti Dream Machine Pro to update my personal
Cloudflare DNS records based on my current public internet IP.

There are tons of tools like this out there, but I prefer to manage things like this
myself.

## Usage

### Prerequisites

Since this is designed for running on a UDM Pro, there are a few pre-requisites you'll want
to take care of. First, the UDM Pro starts with a clean slate on every boot, so you'll want
to install [on-boot-script](https://github.com/boostchicken-dev/udm-utilities/tree/master/on-boot-script)
first. This gives you a mechanism for executing startup scripts on every boot. 

NOTE: I've added some instructions for the UDR/UDM SE below, but keep in mind I've only tested
on the UDM Pro.

### Installing 

With the prereqs out of the way, we can set up `cfdns` three different ways, depending on your preferences:

1. Install this [cronjob helper script](https://github.com/boostchicken-dev/udm-utilities/blob/master/on-boot-script/examples/udm-files/on_boot.d/25-add-cron-jobs.sh)
to execute `cfdns` on a schedule as a one-shot utility. See [the example job file](examples/cfdns) for reference.
2. Start a container via `podman` that runs `cfdns` as a daemon on a configured schedule
3. (UDR/UDM SE only) Install a systemctl service that runs `cfdns` as a daemon. See [the example unit file](examples/cfdns.service) for reference.

If you are on a UDM Pro, I would recommend either option 1 or 2. For a UDR or UDM SE, I would recommend option 3. If I publish a Docker image for
use with podman, it will be tagged as `bitwalker/cfdns:latest`. Since I'm not doing that yet, you'll need to either build the image yourself and
publish it to your own repo, or use option 1.

Make sure you configure `cfdns` as outlined below.

NOTE: I have a UDM Pro, not a UDR or UDM SE, so it is based on what I know of those systems, but is untested.

### Running Manually

The recommended way to use `cfdns` is as an on-boot script, or as a daemon, but in those
situations where you wish to run the tool by hand, see the output of `cfdns help` for usage instructions.

## Configuration

Regardless of how you run `cfdns`, you'll need to configure it. You can pass flags manually, or you can use
a `config.toml` that you must place in one of the following directories, depending on your usage:

* On a UDM Pro: `/mnt/data/cfdns/etc/config.toml`
* On a UDR/UDM SE: `/data/cfdns/etc/config.toml`
* On a desktop system: either `$XDG_CONFIG_HOME/cfdns/config.toml` or `$HOME/.config/cfdns/config.toml`

You can also pass `--config <path>` or export `CONFIG=<path>` in your shell, to override where the config file is pulled from.

The configuration is expressed as a TOML file, and is designed to be flexible enough to support updating DNS records in multiple
zones and potentially multiple accounts. In short, the process looks like this:

1. Define which interfaces to monitor, and on what interval (in seconds) you want to check for changes
2. Configure what zones you are managing. Each zone must be assigned an API token you create [in the dashboard](https://dash.cloudflare.com/profile/api-tokens).
3. Define what records you want to update, by specifying the record details and binding the record to an interface and zone

At runtime, this configuration is transformed into a set of watcher threads, one for each unique combination of interface and API token,
each of which handle updating all of the records bound to that interface and token.

For an example of what a simple configuration looks like, see below:

```toml
# Configures monitoring of the wan0 interface every 15m
[[interfaces]]
name = "wan0"
interval = 900

# Configures the API token to use when updating records in the 'example.com' zone
[[zones]]
name = "example.com"
token = "TOKEN"

# Binds foo.example.com to the wan0 interface address, managed via the 'example.com' zone
[[records]]
interface = "wan0"
zone = "example.com"
# Supported types are A and AAAA (IPv4 vs IPv6 respectively)
type = "A"
name = "foo.example.com"
# The following are optional settings for the DNS record:
# proxied = false
# ttl = 1
```

# License

MIT or Apache 2. Your choice.
