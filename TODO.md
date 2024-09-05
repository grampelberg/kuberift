# TODO

## Documentation

- Get a full architecture explanation together.
- Explain "how it works" for each piece of functionality.

## Authorization

- Groups are probably what most users are going to want to use to configure all
  this. The closest to the OpenID spec would be via adding extra scopes that add
  the data required to the token and then map back to a group. Imagine:

  ```yaml
  user: email
  group: https://myapp.example.com/group
  ```

  The downside to using this kind of configuration is that it'll need to be
  handled in the provider backend and it is unclear how easy that'll be. It is
  possible in auth0, so I'll go down this route for now.

  Note: it looks like Google might require addition verification to get the
  `groups()` scope "externally".

## TUI

- Is there a way to do FPS on a per-session basis with prometheus? Naively the
  way to do it would be to have a per-session label value, but that would be
  crazy for cardinality.

- There's a bug somewhere in `log_stream`. My k3d cluster restarted and while I
  could get all the logs, the stream wouldn't keep running - it'd terminate
  immediately. `stern` seemed to be working fine. Recreating the cluster caused
  everything to work again.

## SFTP

- Document that the permissions here are different than for the dashboard. You
  can get away with `get` and `exec` on ~everything as long as you use `scp`.
  Anything `sftp` is going to do a `readdir` and require `list`.
- The API for `russh_sftp` feels nicer than the one for dashboard currently -
  hand off a channel entirely instead of dealing with `data()` to begin with.
  Should `Dashboard` get reimplemented to take something like
  `async Read + Write` instead? I think I didn't do it this way to being with
  because of writes being consumed entirely.
- Allow globs in file paths, eg `/*/nginx**/etc/passwd`.
- Return an error that is nicer than "no files found" when a container doesn't
  have cat/ls.

## SSH Functionality

- Allow `ssh` directly into a pod without starting the dashboard.
- Enable `ssh -L` for forwarding requests _into_ a pod.
- Enable `ssh -R` for forwarding a remote service _into_ a localhost.

## Ingress Tunnel

## Egress Tunnel

- Display error for when the `tcpip_forward` address is `localhost`.
- Need some way to do cleanup and lifecycle management of endpoints and
  services.

## Build

- Move client_id and config_url to a build-time concern.