# Slack port configuration tool for Arista CloudVision
This tool creates a slack bot that will allow users to interact with CloudVision through chat. The following commands are currently supported:
`/portcheck <walljack>`
`/portup <walljack>`
`/portdown <walljack>`

where `walljack` is a wall jack number that has been tagged to an interface in CloudVision using the tag `wall_jack`

TODO: Insert image

## Options for running:
* Specify all parameters on command line
	`slack-port-config --cvp-host www.cv.arista.io --cvp-port 443 --cvp-token <token> --slack-token <token>`
* Specify a config file with `-c` in TOML
	`slack-port-config -c config.toml`

## Config file example:
```
[cloudvision]
hostname = "www.cv-staging.arista.io"
port = 443
token = "cvptoken"
[slack]
token = "slacktoken"
```
