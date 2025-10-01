<?php
/**
 * Plugin Name: Test Plugin
 * Description: A test WordPress plugin
 */

// Include main functionality
require_once 'includes/functions.php';
require_once 'includes/admin.php';

// Enqueue scripts and styles
function test_plugin_enqueue_scripts() {
    wp_enqueue_script('test-plugin-main', 'assets/js/main.js', array('jquery'), '1.0.0', true);
    wp_enqueue_style('test-plugin-styles', 'assets/css/style.css', array(), '1.0.0');
}
add_action('wp_enqueue_scripts', 'test_plugin_enqueue_scripts');

// This file includes another file in comments
// require_once 'includes/disabled.php';