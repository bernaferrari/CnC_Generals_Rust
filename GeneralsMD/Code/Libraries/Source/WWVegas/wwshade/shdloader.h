#ifndef SHDLOADER_H
#define SHDLOADER_H

#ifndef PROTO_H
#include "proto.h"
#endif

class ShdMeshLoaderClass : public PrototypeLoaderClass
{
public:

	virtual int						Chunk_Type(void) { return W3D_CHUNK_SHDMESH; }
	virtual PrototypeClass*		Load_W3D(ChunkLoadClass& cload);
};

/*
** Prototype loader that converts legacy meshes into Shader meshes
*/
class ShdMeshLegacyLoaderClass : public PrototypeLoaderClass
{
public:

	virtual int						Chunk_Type(void) { return W3D_CHUNK_MESH; }
	virtual PrototypeClass *	Load_W3D(ChunkLoadClass & cload);
};


extern ShdMeshLoaderClass			_ShdMeshLoader;
extern ShdMeshLegacyLoaderClass	_ShdMeshLegacyLoader;

#endif
