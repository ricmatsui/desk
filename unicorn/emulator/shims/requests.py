# requests shim: only spacex_animation uses this, which the emulator does not support.


def get(*args, **kwargs):
    raise NotImplementedError("requests is not available on the emulator")
