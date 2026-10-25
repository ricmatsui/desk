# deflate shim: only spacex_animation uses this, which the emulator does not support.


class DeflateIO:
    def __init__(self, *args, **kwargs):
        raise NotImplementedError("deflate is not available on the emulator")
