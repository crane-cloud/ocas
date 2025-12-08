#!/bin/bash
ansible-playbook -i inventory.yaml yonga-00.yaml

sleep 120
ansible-playbook -i inventory.yaml yonga-01.yaml

sleep 120
ansible-playbook -i inventory.yaml yonga-02.yaml

sleep 120
ansible-playbook -i inventory.yaml yonga-03.yaml

sleep 120
ansible-playbook -i inventory.yaml yonga-00.yaml
