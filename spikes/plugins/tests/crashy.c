/* A fake VST3 module whose factory crashes, to prove the scan survives it. */
int ModuleEntry(void *h) { (void)h; return 1; }
int ModuleExit(void) { return 1; }
void *GetPluginFactory(void) { volatile int *p = 0; *p = 1; return 0; }
