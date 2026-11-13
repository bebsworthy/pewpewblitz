package com.pewpewblitz.client;

import android.Manifest;
import android.app.AlertDialog;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;
import android.view.WindowInsets;
import android.view.WindowInsetsController;

import com.google.androidgamesdk.GameActivity;

/**
 * Hosts the Rust/Bevy client ({@code libbrawler_android.so}) full screen in landscape and owns the
 * Android 17 local-network permission, which servers on the player's Wi-Fi require.
 */
public class MainActivity extends GameActivity {
    private static final int LOCAL_NETWORK_REQUEST = 1;

    static {
        System.loadLibrary("brawler_android");
    }

    /** True once the player chose "Not now"; asked again on the next app launch. */
    private boolean localNetworkDeclined;
    private boolean localNetworkRequestPending;
    private AlertDialog localNetworkDialog;

    @Override
    protected void onResume() {
        super.onResume();
        requestLocalNetworkIfNeeded();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            WindowInsetsController controller = getWindow().getInsetsController();
            if (controller != null) {
                controller.hide(WindowInsets.Type.systemBars());
                controller.setSystemBarsBehavior(
                        WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        }
    }

    private boolean hasLocalNetwork() {
        return Build.VERSION.SDK_INT < 37
                || checkSelfPermission(Manifest.permission.ACCESS_LOCAL_NETWORK)
                        == PackageManager.PERMISSION_GRANTED;
    }

    /** Requests the permission at launch and on return from Settings, before any connection. */
    private void requestLocalNetworkIfNeeded() {
        if (hasLocalNetwork()) {
            dismissLocalNetworkDialog();
            return;
        }
        if (localNetworkDeclined || localNetworkRequestPending || isDialogShowing()) {
            return;
        }
        localNetworkRequestPending = true;
        requestPermissions(
                new String[] {Manifest.permission.ACCESS_LOCAL_NETWORK}, LOCAL_NETWORK_REQUEST);
    }

    @Override
    public void onRequestPermissionsResult(
            int requestCode, String[] permissions, int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        if (requestCode != LOCAL_NETWORK_REQUEST) {
            return;
        }
        localNetworkRequestPending = false;
        if (!hasLocalNetwork()) {
            showLocalNetworkDenied();
        }
    }

    private void showLocalNetworkDenied() {
        // After repeated denials Android stops showing the prompt; retry then opens Settings.
        boolean canPromptAgain =
                shouldShowRequestPermissionRationale(Manifest.permission.ACCESS_LOCAL_NETWORK);
        localNetworkDialog = new AlertDialog.Builder(this)
                .setTitle("Local network access needed")
                .setMessage("PewPew Blitz connects to game servers on your Wi-Fi network. "
                        + "Without local network access it cannot reach a server running on "
                        + "your computer.")
                .setCancelable(false)
                .setPositiveButton(canPromptAgain ? "Try again" : "Open settings",
                        (dialog, which) -> {
                            localNetworkDialog = null;
                            if (canPromptAgain) {
                                requestLocalNetworkIfNeeded();
                            } else {
                                startActivity(new Intent(
                                        Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                                        Uri.fromParts("package", getPackageName(), null)));
                            }
                        })
                .setNegativeButton("Not now", (dialog, which) -> localNetworkDeclined = true)
                .show();
    }

    private boolean isDialogShowing() {
        return localNetworkDialog != null && localNetworkDialog.isShowing();
    }

    private void dismissLocalNetworkDialog() {
        if (isDialogShowing()) {
            localNetworkDialog.dismiss();
        }
        localNetworkDialog = null;
    }
}
