use log::{info, trace, warn, Level};
use netmod_mem::MemMod;
use ratman::{Identity, Router};

use android_logger::Config;

// Run ratman for the simple android test.
async fn router_testing() -> Result<(), Box<dyn Error>> {
    // Build a simple channel in memory
    let mm1 = MemMod::new();

    // Initialise one router
    let r1 = Router::new();

    // Add channel endpoint to router
    r1.add_endpoint(mm1).await;

    // Create a user and add them to the router
    let u1 = Identity::random();
    r1.add_user(u1).await?;

    // And mark router "online"
    r1.online(u1).await?;

    // The routers will now start announcing their new users on the
    // micro-network.  You can now poll for new user discoveries.
    r1.discover().await;

    // This test needs two android devices that are connected
    // via Wifi-Direct.
    // device1 needs to install .apk [android-vpn app]
    // which contains this library.[libratman_android.so]
    //
    // Device2 should be able to register ratcat to the r1(router)
    // via termux or adb by ./ratcat --register
    // * expected output on the device2:
    // $ Registered address: [...]
    // $ Registered a new address!  You may now run `ratcat` to send data
    Ok(())
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn Java_org_irdest_IrdestVPN_Ratmand_ratrun(
    env: JNIEnv,
    _: JClass,
    _test_string: JString,
) -> jstring {
    // Ignoring the test_string which comes from the android application.

    // Run ratman router for the test
    // TODO: wrap this in async_std::task::block_on
    // let _ = router_testing();
    // let _ = ratrun_with_startup();
    //
    android_logger::init_once(Config::default().with_min_level(Level::Trace));

    trace!("Trace from rust!!!");
    info!("Info from rust!!!");
    warn!("This is warring from rust!!!");

    let mut cfg = daemon::config::Config::new();
    cfg.accept_unknown_peers = true;

    async_std::task::block_on(ratman::daemon::startup::run_app(cfg)).unwrap();

    env.new_string("Testing is running 🐲")
        .expect("Error: can't not make java string!")
        .into_inner()
}
=======
use log::{info, trace, Level};
use netmod_mem::MemMod;
use ratman::{Identity, Router};

use android_logger::Config;

// Run ratman for the simple android test.
async fn router_testing() -> Result<(), Box<dyn Error>> {
    // Build a simple channel in memory
    let mm1 = MemMod::new();

    // Initialise one router
    let r1 = Router::new();

    // Add channel endpoint to router
    r1.add_endpoint(mm1).await;

    // Create a user and add them to the router
    let u1 = Identity::random();
    r1.add_user(u1).await?;

    // And mark router "online"
    r1.online(u1).await?;

    // The routers will now start announcing their new users on the
    // micro-network.  You can now poll for new user discoveries.
    r1.discover().await;

    // This test needs two android devices that are connected
    // via Wifi-Direct.
    // device1 needs to install .apk [android-vpn app]
    // which contains this library.[libratman_android.so]
    //
    // Device2 should be able to register ratcat to the r1(router)
    // via termux or adb by ./ratcat --register
    // * expected output on the device2:
    // $ Registered address: [...]
    // $ Registered a new address!  You may now run `ratcat` to send data
    Ok(())
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn Java_org_irdest_IrdestVPN_Ratmand_ratrun(
    env: JNIEnv,
    _: JClass,
    _test_string: JString,
) -> jstring {
    // Ignoring the test_string which comes from the android application.

    // Send log to logcat
    android_logger::init_once(
        Config::default()
            .with_tag("ratmand-android-logger")
            .with_min_level(Level::Trace),
    );

    let mut cfg = daemon::config::Config::new();
    cfg.accept_unknown_peers = true;

    info!("@android-dev#: config => {:?}", cfg);

    async_std::task::block_on(ratman::daemon::startup::run_app(cfg)).unwrap();

    env.new_string("Testing is running 🐲")
        .expect("Error: can't not make java string!")
        .into_inner()
}
