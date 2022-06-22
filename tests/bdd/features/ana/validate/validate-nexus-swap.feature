Feature: Swap ANA enabled Nexus on ANA enabled host

  Background:
    Given a control plane, 2 ANA-enabled Io-Engine instances, 1 ANA-enabled host and a published volume

  Scenario: replace failed I/O path on demand for NVMe controller
    Given a client connected to one nexus via single I/O path
    And fio client is running against target nexus
    When the only I/O path degrades
    Then it should be possible to create a second nexus and connect it as the second path
    And it should be possible to remove the first failed I/O path
    And fio client should successfully complete with the replaced I/O path
