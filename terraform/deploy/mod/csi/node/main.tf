variable "image" {}
variable "tag" {}
variable "registry" {}
variable "grace_period" {}
variable "registrar_image" {}
variable "rust_log" {}
variable "io_queues" {}
variable "credentials" {}
variable "namespace" {}
variable "product_name_prefix" {}
variable "image_pull_policy" {}

resource "kubernetes_daemonset" "csi_node" {
  metadata {
    name      = "csi-node"
    namespace = var.namespace
  }

  spec {
    selector {
      match_labels = {
        app = "csi-node"
      }
    }

    template {
      metadata {
        labels = {
          app = "csi-node"
        }
      }

      spec {
        image_pull_secrets {
          name = var.credentials
        }
        service_account_name             = "${var.product_name_prefix}service-account"
        termination_grace_period_seconds = var.grace_period

        volume {
          name = "device"

          host_path {
            path = "/dev"
            type = "Directory"
          }
        }

        volume {
          name = "sys"

          host_path {
            path = "/sys"
            type = "Directory"
          }
        }

        volume {
          name = "run-udev"

          host_path {
            path = "/run/udev"
            type = "Directory"
          }
        }

        volume {
          name = "registration-dir"

          host_path {
            path = "/var/lib/kubelet/plugins_registry/"
            type = "Directory"
          }
        }

        volume {
          name = "plugin-dir"

          host_path {
            path = "/var/lib/kubelet/plugins/io.openebs.mayastor/"
            type = "DirectoryOrCreate"
          }
        }

        volume {
          name = "kubelet-dir"

          host_path {
            path = "/var/lib/kubelet"
            type = "Directory"
          }
        }

        container {
          name  = "csi-node"
          image = format("%s/%s:%s", var.registry, var.image, var.tag)
          args = [
            "--csi-socket=/csi/csi.sock",
            "--node-name=$(MY_NODE_NAME)",
            "--grpc-endpoint=$(MY_POD_IP):10199",
            format("--nvme-nr-io-queues=%s", var.io_queues),
            "-v"
          ]

          env {
            name = "MY_NODE_NAME"

            value_from {
              field_ref {
                field_path = "spec.nodeName"
              }
            }
          }

          env {
            name = "MY_POD_IP"

            value_from {
              field_ref {
                field_path = "status.podIP"
              }
            }
          }

          env {
            name  = "RUST_BACKTRACE"
            value = "1"
          }


          volume_mount {
            name       = "device"
            mount_path = "/dev"
          }

          volume_mount {
            name       = "sys"
            mount_path = "/sys"
          }

          volume_mount {
            name       = "run-udev"
            mount_path = "/run/udev"
          }

          volume_mount {
            name       = "plugin-dir"
            mount_path = "/csi"
          }

          volume_mount {
            name              = "kubelet-dir"
            mount_path        = "/var/lib/kubelet"
            mount_propagation = "Bidirectional"
          }

          image_pull_policy = var.image_pull_policy

          security_context {
            privileged = true
          }
        }

        container {
          name  = "csi-driver-registrar"
          image = var.registrar_image
          args = [
            "--csi-address=/csi/csi.sock",
            "--kubelet-registration-path=/var/lib/kubelet/plugins/io.openebs.mayastor/csi.sock"
          ]

          volume_mount {
            name       = "plugin-dir"
            mount_path = "/csi"
          }

          volume_mount {
            name       = "registration-dir"
            mount_path = "/registration"
          }
        }

        node_selector = {
          "kubernetes.io/arch" = "amd64"
        }

        host_network = true
      }
    }

    strategy {
      type = "RollingUpdate"

      rolling_update {
        max_unavailable = "1"
      }
    }

    min_ready_seconds = 10
  }
}
