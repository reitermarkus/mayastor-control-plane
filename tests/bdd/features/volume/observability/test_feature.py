"""Volume observability feature tests."""

from pytest_bdd import (
    given,
    scenario,
    then,
    when,
)

import pytest

from common.deployer import Deployer
from common.apiclient import ApiClient

from openapi.model.create_pool_body import CreatePoolBody
from openapi.model.create_volume_body import CreateVolumeBody
from openapi.model.volume_spec import VolumeSpec
from openapi.model.volume_state import VolumeState
from openapi.model.volume_status import VolumeStatus
from openapi.model.spec_status import SpecStatus
from openapi.model.volume_policy import VolumePolicy
from openapi.model.replica_state import ReplicaState
from openapi.model.replica_topology import ReplicaTopology


POOL_UUID = "4cc6ee64-7232-497d-a26f-38284a444980"
VOLUME_UUID = "5cd5378e-3f05-47f1-a830-a0f5873a1449"
NODE_NAME = "io-engine-1"
VOLUME_CTX_KEY = "volume"
VOLUME_SIZE = 10485761


# This fixture will be automatically used by all tests.
# It starts the deployer which launches all the necessary containers.
# A pool and volume are created for convenience such that it is available for use by the tests.
@pytest.fixture(autouse=True, scope="module")
def init():
    Deployer.start(1)
    ApiClient.pools_api().put_node_pool(
        NODE_NAME, POOL_UUID, CreatePoolBody(["malloc:///disk?size_mb=50"])
    )
    ApiClient.volumes_api().put_volume(
        VOLUME_UUID, CreateVolumeBody(VolumePolicy(False), 1, VOLUME_SIZE)
    )
    yield
    Deployer.stop()


# Fixture used to pass the volume context between test steps.
@pytest.fixture(scope="function")
def volume_ctx():
    return {}


@scenario("feature.feature", "requesting volume information")
def test_requesting_volume_information():
    """requesting volume information."""


@given("an existing volume")
def an_existing_volume():
    """an existing volume."""
    volume = ApiClient.volumes_api().get_volume(VOLUME_UUID)
    assert volume.spec.uuid == VOLUME_UUID


@when("a user issues a GET request for a volume")
def a_user_issues_a_get_request_for_a_volume(volume_ctx):
    """a user issues a GET request for a volume."""
    volume_ctx[VOLUME_CTX_KEY] = ApiClient.volumes_api().get_volume(VOLUME_UUID)


@then("a volume object representing the volume should be returned")
def a_volume_object_representing_the_volume_should_be_returned(volume_ctx):
    """a volume object representing the volume should be returned."""
    expected_spec = VolumeSpec(
        1,
        VOLUME_SIZE,
        SpecStatus("Created"),
        VOLUME_UUID,
        VolumePolicy(False),
    )

    volume = volume_ctx[VOLUME_CTX_KEY]
    assert str(volume.spec) == str(expected_spec)

    # The key for the replica topology is the replica UUID. This is assigned at replica creation
    # time, so get the replica UUID from the returned volume object, and use this as the key of
    # the expected replica topology.
    expected_replica_toplogy = {}
    for key, value in volume.state.replica_topology.items():
        expected_replica_toplogy[key] = ReplicaTopology(
            ReplicaState("Online"), node="io-engine-1", pool=POOL_UUID
        )
    expected_state = VolumeState(
        VOLUME_SIZE,
        VolumeStatus("Online"),
        VOLUME_UUID,
        expected_replica_toplogy,
    )
    assert str(volume.state) == str(expected_state)
