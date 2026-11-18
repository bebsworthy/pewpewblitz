package com.pewpewblitz.client;

import android.Manifest;
import android.app.AlertDialog;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.graphics.Insets;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Process;
import android.provider.Settings;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;

import androidx.activity.OnBackPressedCallback;

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
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        keepSurfaceOutOfDisplayCutout();
        routeBackToGame();
    }

    /**
     * Back means "back" in the game (BRL-0198). At targetSdk 37 the gesture arrives only through
     * the back-invoked dispatcher, whose default finishes this activity while the native game
     * keeps running. The game decides: an inner screen consumes it, a root screen returns false
     * and the task moves to the background like any root activity, keeping the session.
     */
    private void routeBackToGame() {
        getOnBackPressedDispatcher().addCallback(this, new OnBackPressedCallback(true) {
            @Override
            public void handleOnBackPressed() {
                if (!nativeOnBackPressed()) {
                    moveTaskToBack(true);
                }
            }
        });
    }

    /** Implemented by {@code libbrawler_android.so}; never blocks on the game loop. */
    private native boolean nativeOnBackPressed();

    /**
     * The on-screen keyboard's action key (Done) confirms the edited field (BRL-06pz). The game
     * loop's GameActivity glue drops this event, so it is forwarded to the game directly.
     */
    @Override
    public void onEditorAction(int action) {
        super.onEditorAction(action);
        nativeOnImeAction();
    }

    /**
     * Hiding the keyboard with Back or its own control cancels the edited field, as Back does
     * elsewhere in the game (BRL-06pz).
     */
    @Override
    public void onSoftwareKeyboardVisibilityChanged(boolean visible) {
        super.onSoftwareKeyboardVisibilityChanged(visible);
        nativeOnSoftKeyboardVisibilityChanged(visible);
    }

    /** Implemented by {@code libbrawler_android.so}; only records the press. */
    private native void nativeOnImeAction();

    /** Implemented by {@code libbrawler_android.so}; only records the change. */
    private native void nativeOnSoftKeyboardVisibilityChanged(boolean visible);

    /**
     * The native game loop runs once per process: winit cannot recreate its event loop, so a new
     * activity in this process would hang on the splash. Once this activity is finishing (QUIT
     * returned from the game loop, or the system finished it) the process ends with it, and the
     * next launch starts fresh (BRL-0198).
     */
    @Override
    protected void onDestroy() {
        super.onDestroy();
        if (isFinishing()) {
            Process.killProcess(Process.myPid());
        }
    }

    /**
     * Edge-to-edge is mandatory from API 35, so the game surface would otherwise extend under the
     * camera cutout and hide shell and HUD controls there. Padding the content view by the cutout
     * insets keeps the whole Bevy window in the safe area; the strip shows black (BRL-0196).
     */
    private void keepSurfaceOutOfDisplayCutout() {
        View content = findViewById(android.R.id.content);
        if (content == null) {
            return;
        }
        content.setBackgroundColor(Color.BLACK);
        content.setOnApplyWindowInsetsListener((view, insets) -> {
            Insets cutout = insets.getInsets(WindowInsets.Type.displayCutout());
            view.setPadding(cutout.left, cutout.top, cutout.right, cutout.bottom);
            return insets;
        });
        content.requestApplyInsets();
    }

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
