// header file for use in wdump util only

#ifndef SHDDUMP_H
#define SHDDUMP_H

#ifndef SHDCLASSIDS_H
#include "shdclassids.h"
#endif

const char* Shader_ClassIDs[SHDDEF_CLASSID_LAST]=
{
	"SHDDEF_CLASSID_DUMMY",
	"SHDDEF_CLASSID_SIMPLE",
	"SHDDEF_CLASSID_GLOSSMASK",
	"SHDDEF_CLASSID_BUMPSPEC",
	"SHDDEF_CLASSID_BUMPDIFF",
	"SHDDEF_CLASSID_CUBEMAP"
};

struct ShdDef_ChunkStruct
{
	DWORD DefChunkId;
	char	Pad0[6];
	char  DefName[8];
	char	Pad1[2];
	int	SurfaceType;
	DWORD ShdChunkId;
	char	Pad2[6];
	char	TexName[];
};


#endif //SHDDUMP_H
