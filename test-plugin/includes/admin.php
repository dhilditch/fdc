<?php

function test_plugin_admin_menu() {
    add_menu_page('Test Plugin', 'Test Plugin', 'manage_options', 'test-plugin');
}