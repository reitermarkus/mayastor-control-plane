"""Rebuilding a volume feature tests."""

from pytest_bdd import (
    given,
    scenario,
    then,
    when,
)

import pytest
import http
import time
from retrying import retry

from common.deployer import Deployer
from common.apiclient import ApiClient
from common.docker import Docker

from openapi.model.create_pool_body import CreatePoolBody
from openapi.model.create_volume_body import CreateVolumeBody
from openapi.model.protocol import Protocol
from openapi.exceptions import ApiException
from openapi.model.volume_status import VolumeStatus
from openapi.model.volume_policy import VolumePolicy

VOLUME_UUID = "5cd5378e-3f05-47f1-a830-a0f5873a1449"
VOLUME_SIZE = 10485761
NUM_VOLUME_REPLICAS = 2
NODE_1_NAME = "io-engine-1"
NODE_2_NAME = "io-engine-2"
NODE_3_NAME = "io-engine-3"
POOL_1_UUID = "4cc6ee64-7232-497d-a26f-38284a444980"
POOL_2_UUID = "91a60318-bcfe-4e36-92cb-ddc7abf212ea"
POOL_3_UUID = "4d471e62-ca17-44d1-a6d3-8820f6156c1a"
RECONCILE_PERIOD_SECS = 1
MAX_REBUILDS = 0  # Prevent all rebuilds


@pytest.fixture(autouse=True)
def init():
    Deployer.start(
        io_engines="3",
        wait="10s",
        reconcile_period=f"{RECONCILE_PERIOD_SECS}s",
        cache_period="1s",
        max_rebuilds=f"{MAX_REBUILDS}",
    )

    # Only create 2 pools so we can control where the intial replicas are placed.
    ApiClient.pools_api().put_node_pool(
        NODE_1_NAME, POOL_1_UUID, CreatePoolBody(["malloc:///disk?size_mb=50"])
    )
    ApiClient.pools_api().put_node_pool(
        NODE_2_NAME, POOL_2_UUID, CreatePoolBody(["malloc:///disk?size_mb=50"])
    )

    yield
    Deployer.stop()


@scenario(
    "feature.feature",
    "exceeding the maximum number of rebuilds when increasing the replica count",
)
def test_exceeding_the_maximum_number_of_rebuilds_when_increasing_the_replica_count():
    """exceeding the maximum number of rebuilds when increasing the replica count."""


@scenario(
    "feature.feature",
    "exceeding the maximum number of rebuilds when replacing a replica",
)
def test_exceeding_the_maximum_number_of_rebuilds_when_replacing_a_replica():
    """exceeding the maximum number of rebuilds when replacing a replica."""


@given("a user defined maximum number of rebuilds")
def a_user_defined_maximum_number_of_rebuilds():
    """a user defined maximum number of rebuilds."""


@given("an existing published volume")
def an_existing_published_volume():
    """an existing published volume."""
    request = CreateVolumeBody(VolumePolicy(True), NUM_VOLUME_REPLICAS, VOLUME_SIZE)
    ApiClient.volumes_api().put_volume(VOLUME_UUID, request)
    ApiClient.volumes_api().put_volume_target(
        VOLUME_UUID, NODE_1_NAME, Protocol("nvmf")
    )

    # Now the volume has been created, create the additional pool.
    ApiClient.pools_api().put_node_pool(
        NODE_3_NAME, POOL_3_UUID, CreatePoolBody(["malloc:///disk?size_mb=50"])
    )


@when("a replica is faulted")
def a_replica_is_faulted():
    """a replica is faulted."""
    # Fault a replica by stopping the container with the replica.
    # Check the replica becomes unhealthy by waiting for the volume to become degraded.
    Docker.stop_container(NODE_2_NAME)
    wait_for_degraded_volume()


@then(
    "adding a replica should fail if doing so would exceed the maximum number of rebuilds"
)
def adding_a_replica_should_fail_if_doing_so_would_exceed_the_maximum_number_of_rebuilds():
    """adding a replica should fail if doing so would exceed the maximum number of rebuilds."""
    pass
    try:
        ApiClient.volumes_api().put_volume_replica_count(
            VOLUME_UUID, NUM_VOLUME_REPLICAS + 1
        )
    except ApiException as e:
        assert e.status == http.HTTPStatus.INSUFFICIENT_STORAGE


@then(
    "replacing the replica should fail if doing so would exceed the maximum number of rebuilds"
)
def replacing_the_replica_should_fail_if_doing_so_would_exceed_the_maximum_number_of_rebuilds():
    """replacing the replica should fail if doing so would exceed the maximum number of rebuilds."""
    wait_for_replica_removal()
    # Check that a replica doesn't get added to the volume.
    # This should be prevented because it would exceed the number of max rebuilds.
    for _ in range(10):
        check_replica_not_added()
        time.sleep(RECONCILE_PERIOD_SECS)


@retry(wait_fixed=1000, stop_max_attempt_number=10)
def wait_for_degraded_volume():
    volume = ApiClient.volumes_api().get_volume(VOLUME_UUID)
    assert volume.state.status == VolumeStatus("Degraded")


@retry(wait_fixed=1000, stop_max_attempt_number=10)
def wait_for_replica_removal():
    volume = ApiClient.volumes_api().get_volume(VOLUME_UUID)
    assert len(volume.state.target["children"]) == NUM_VOLUME_REPLICAS - 1


def check_replica_not_added():
    volume = ApiClient.volumes_api().get_volume(VOLUME_UUID)
    assert len(volume.state.target["children"]) < NUM_VOLUME_REPLICAS
