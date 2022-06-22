import os
import subprocess

from dataclasses import dataclass


@dataclass
class StartOptions:
    io_engines: int = 1
    wait: str = "10s"
    csi: bool = False
    reconcile_period: str = ""
    cache_period: str = ""
    io_engine_env: str = ""
    agents_env: str = ""
    node_deadline: str = ""
    jaeger: bool = True
    cluster_uid: str = "bdd"
    extra_args: [str] = ()
    rest_env: str = ""
    max_rebuilds: str = ""

    def args(self):
        args = [
            "--io-engines",
            str(self.io_engines),
            "--wait-timeout",
            self.wait,
        ]
        if self.csi:
            args.append("--csi")
        if self.jaeger:
            args.append("--jaeger")
        if len(self.reconcile_period) > 0:
            args.append(f"--reconcile-period={self.reconcile_period}")
            args.append(f"--reconcile-idle-period={self.reconcile_period}")
        if len(self.cache_period) > 0:
            args.append(f"--cache-period={self.cache_period}")
        if len(self.node_deadline) > 0:
            args.append(f"--node-deadline={self.node_deadline}")
        if len(self.io_engine_env) > 0:
            args.append(f"--io-engine-env={self.io_engine_env}")
        if len(self.agents_env) > 0:
            args.append(f"--agents-env={self.agents_env}")
        if len(self.cluster_uid) > 0:
            args.append(f"--cluster-uid={self.cluster_uid}")
        if len(self.rest_env) > 0:
            args.append(f"--rest-env={self.rest_env}")
        if len(self.extra_args) > 0:
            args.append(self.extra_args)
        if len(self.max_rebuilds) > 0:
            args.append(f"--max-rebuilds={self.max_rebuilds}")
        return args


class Deployer(object):
    # Start containers with the provided arguments
    @staticmethod
    def start(
        io_engines=2,
        wait="10s",
        csi=False,
        reconcile_period="",
        cache_period="",
        io_engine_env="",
        agents_env="",
        rest_env="",
        node_deadline="",
        jaeger=True,
        max_rebuilds="",
    ):
        options = StartOptions(
            io_engines,
            wait,
            csi=csi,
            reconcile_period=reconcile_period,
            cache_period=cache_period,
            io_engine_env=io_engine_env,
            agents_env=agents_env,
            node_deadline=node_deadline,
            jaeger=jaeger,
            max_rebuilds=max_rebuilds,
        )
        Deployer.start_with_opts(options)

    # Start containers with the provided options.
    @staticmethod
    def start_with_opts(options: StartOptions):
        deployer_path = os.environ["ROOT_DIR"] + "/target/debug/deployer"
        subprocess.run([deployer_path, "start"] + options.args())

    # Stop containers
    @staticmethod
    def stop():
        if os.getenv("CLEAN") == "false":
            return
        deployer_path = os.environ["ROOT_DIR"] + "/target/debug/deployer"
        subprocess.run([deployer_path, "stop"])
