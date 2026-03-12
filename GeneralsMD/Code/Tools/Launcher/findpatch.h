#ifndef FINDPATCH_HEADER
#define FINDPATCH_HEADER

#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <direct.h>
#include "wstypes.h"
#include "configfile.h"

int Find_Patch(OUT char *file,int maxlen, ConfigFile &config);
bit8 Get_App_Dir(OUT char *file,int maxlen, ConfigFile &config, int index);
void Delete_Patches(ConfigFile &config);

#endif