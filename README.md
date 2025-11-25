# K8s Post-success labels

A command wrapper that updates a K8s resource's label after the wrapped command
succeeds. It's main use is to provide an easy way to annotate resources for
Argo-Events policies to mark plain sensors as successfully completed.

If the wrapped command fails no label update will be attempted. If the wrapper
fails to update the label even though the wrapped command succeeded, the return
code depends on the nature of the error:

- it will be `66` if the resource can't be found or the label update couldn't be
  performed
- it will be `68` if the wrapper can't connect to the Kubernetes API

## Usage

```bash
k8s-psl -n <namespace> -l <label>=<value> (job|pod)/<name> -- [command-to-wrap] [...command-arguments]
```

Example:

```bash
k8s-psl -n batch-processing -l succeeded=true job/do-the-stuff -- echo "I did all of the stuffs"
```
