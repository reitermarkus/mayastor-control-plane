"""Volume creation feature tests."""
import time

from pytest_bdd import (
    given,
    scenario,
    then,
    when,
)

import pytest
import docker
import requests

from common.deployer import Deployer
from common.apiclient import ApiClient
from common.docker import Docker

from openapi.model.create_pool_body import CreatePoolBody
from openapi.model.create_volume_body import CreateVolumeBody
from openapi.model.volume_spec import VolumeSpec
from openapi.model.spec_status import SpecStatus
from openapi.model.volume_state import VolumeState
from openapi.model.volume_status import VolumeStatus
from openapi.model.volume_policy import VolumePolicy
from openapi.model.replica_state import ReplicaState
from openapi.model.replica_topology import ReplicaTopology

VOLUME_UUID = "5cd5378e-3f05-47f1-a830-a0f5873a1449"
VOLUME_SIZE = 10485761
NUM_VOLUME_REPLICAS = 1
CREATE_REQUEST_KEY = "create_request"
POOL_UUID = "4cc6ee64-7232-497d-a26f-38284a444980"
NODE_NAME = "io-engine-1"


# This fixture will be automatically used by all tests.
# It starts the deployer which launches all the necessary containers.
# A pool is created for convenience such that it is available for use by the tests.
@pytest.fixture(autouse=True)
def init():
    Deployer.start(1)
    ApiClient.pools_api().put_node_pool(
        NODE_NAME, POOL_UUID, CreatePoolBody(["malloc:///disk?size_mb=50"])
    )
    yield
    Deployer.stop()


# Fixture used to pass the volume create request between test steps.
@pytest.fixture(scope="function")
def create_request():
    return {}


@scenario("feature.feature", "provisioning failure due to missing Io-Engine")
def test_provisioning_failure_due_to_missing_io_engine():
    """provisioning failure."""


@scenario("feature.feature", "provisioning failure due to gRPC timeout")
def test_provisioning_failure_due_to_grpc_timeout():
    """provisioning failure."""


@scenario("feature.feature", "desired number of replicas cannot be created")
def test_desired_number_of_replicas_cannot_be_created():
    """desired number of replicas cannot be created."""


@scenario("feature.feature", "sufficient suitable pools")
def test_sufficient_suitable_pools():
    """sufficient suitable pools."""


@given("a control plane, Io-Engine instances and a pool")
def a_control_plane_io_engine_instances_and_a_pool():
    """a control plane, Io-Engine instances and a pool."""
    docker_client = docker.from_env()

    # The control plane comprises the core agents, rest server and etcd instance.
    for component in ["core", "rest", "etcd"]:
        Docker.check_container_running(component)

    # Check all Io-Engine instances are running
    try:
        io_engines = docker_client.containers.list(
            all=True, filters={"name": "io-engine"}
        )
    except docker.errors.NotFound:
        raise Exception("No Io-Engine instances")

    for io_engine in io_engines:
        Docker.check_container_running(io_engine.attrs["Name"])

    # Check for a pool
    pool = ApiClient.pools_api().get_pool(POOL_UUID)
    assert pool.id == POOL_UUID


@given("a request for a volume")
def a_request_for_a_volume(create_request):
    """a request for a volume."""
    request = CreateVolumeBody(VolumePolicy(False), NUM_VOLUME_REPLICAS, VOLUME_SIZE)
    create_request[CREATE_REQUEST_KEY] = request


@when("a create operation takes longer than the gRPC timeout")
def a_create_operation_takes_longer_than_the_grpc_timeout():
    """a create operation takes longer than the gRPC timeout."""
    # Delete the Io-Engine instances to ensure the operation can't complete and so takes longer
    # than the gRPC timeout.
    docker_client = docker.from_env()
    try:
        io_engines = docker_client.containers.list(
            all=True, filters={"name": "io-engine"}
        )
    except docker.errors.NotFound:
        raise Exception("No Io-Engine instances")

    for io_engine in io_engines:
        io_engine.kill()


@when("the number of suitable pools is less than the number of desired volume replicas")
def the_number_of_suitable_pools_is_less_than_the_number_of_desired_volume_replicas(
    create_request,
):
    """the number of suitable pools is less than the number of desired volume replicas."""
    # Delete the pool so that there aren't enough
    pools_api = ApiClient.pools_api()
    pools_api.del_pool(POOL_UUID)
    num_pools = len(pools_api.get_pools())
    num_volume_replicas = create_request[CREATE_REQUEST_KEY]["replicas"]
    assert num_pools < num_volume_replicas


@when(
    "the number of volume replicas is less than or equal to the number of suitable pools"
)
def the_number_of_volume_replicas_is_less_than_or_equal_to_the_number_of_suitable_pools(
    create_request,
):
    """the number of volume replicas is less than or equal to the number of suitable pools."""
    num_pools = len(ApiClient.pools_api().get_pools())
    num_volume_replicas = create_request[CREATE_REQUEST_KEY]["replicas"]
    assert num_volume_replicas <= num_pools


@when("there are no available Io-Engine instances")
def there_are_no_available_io_engine_instances():
    """there are no available Io-Engine instances."""
    # Kill io-engine instance
    docker_client = docker.from_env()
    container = docker_client.containers.get("io-engine-1")
    container.kill()


@then("there should not be any specs relating to the volume")
def there_should_not_be_any_specs_relating_to_the_volume():
    """there should not be any specs relating to the volume."""
    # Restart the core agent so that all the specs are reloaded from the persistent store.
    global specs
    docker_client = docker.from_env()
    core = docker_client.containers.get("core")
    core.restart()

    # After restarting the core container it can take a little time to refresh the specs,
    # so retry a number of times.
    for i in range(5):
        try:
            specs = ApiClient.specs_api().get_specs()
            break
        except:
            time.sleep(1)

    assert len(specs["volumes"]) == 0
    assert len(specs["nexuses"]) == 0
    assert len(specs["replicas"]) == 0


@then("volume creation should fail with a precondition failed error")
def volume_creation_should_fail_with_a_precondition_failed_error(create_request):
    """volume creation should fail."""
    request = create_request[CREATE_REQUEST_KEY]
    try:
        ApiClient.volumes_api().put_volume(VOLUME_UUID, request)
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["precondition_failed"]

    # Check that the volume wasn't created.
    volumes = ApiClient.volumes_api().get_volumes().entries
    assert len(volumes) == 0


@then("volume creation should fail with an insufficient storage error")
def volume_creation_should_fail_with_an_insufficient_storage_error(create_request):
    """volume creation should fail with an insufficient storage error."""
    request = create_request[CREATE_REQUEST_KEY]
    try:
        ApiClient.volumes_api().put_volume(VOLUME_UUID, request)
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["insufficient_storage"]
    finally:
        # Check that the volume wasn't created.
        volumes = ApiClient.volumes_api().get_volumes().entries
        assert len(volumes) == 0


@then("volume creation should succeed with a returned volume object")
def volume_creation_should_succeed_with_a_returned_volume_object(create_request):
    """volume creation should succeed with a returned volume object."""
    expected_spec = VolumeSpec(
        1,
        VOLUME_SIZE,
        SpecStatus("Created"),
        VOLUME_UUID,
        VolumePolicy(False),
    )

    # Check the volume object returned is as expected
    request = create_request[CREATE_REQUEST_KEY]
    volume = ApiClient.volumes_api().put_volume(VOLUME_UUID, request)
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
