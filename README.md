# K8s Post-success labels

A command wrapper that updates a K8s resource's label after the wrapped command
succeeds. It's main use is to provide an easy way to annotate resources for
Argo-Events policies to mark plain sensors as successfully completed.
