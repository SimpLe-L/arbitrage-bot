// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {AvaxArbExecutor} from "../src/AvaxArbExecutor.sol";

contract DeployScript is Script {
    function setUp() public {}

    function run() public {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(deployerPrivateKey);

        // 部署套利合约
        AvaxArbExecutor arbExecutor = new AvaxArbExecutor();

        console.log("AvaxArbExecutor deployed at:", address(arbExecutor));
        console.log("Owner:", arbExecutor.owner());

        vm.stopBroadcast();
    }
}
