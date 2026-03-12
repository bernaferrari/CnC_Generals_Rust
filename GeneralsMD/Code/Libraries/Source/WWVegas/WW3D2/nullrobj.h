#if defined(_MSC_VER)
#pragma once
#endif

#ifndef NULL_H
#define NULL_H

#ifndef RENDOBJ_H
#include "rendobj.h"
#endif

#include "proto.h"

class Null3DObjClass : public RenderObjClass
{
public:

	Null3DObjClass(const char * name = "NULL");
	Null3DObjClass(const Null3DObjClass & src);
	Null3DObjClass & operator = (const Null3DObjClass & that);
			
	virtual int						Class_ID(void) const;
	virtual RenderObjClass *	Clone(void) const;
	virtual const char *			Get_Name(void) const						{ return Name; }
	virtual void					Render(RenderInfoClass & rinfo);
	virtual void					Get_Obj_Space_Bounding_Sphere(SphereClass & sphere) const;
	virtual void					Get_Obj_Space_Bounding_Box(AABoxClass & box) const;

protected:

	char								Name[2*W3D_NAME_LEN];
};


class NullPrototypeClass : public W3DMPO, public PrototypeClass
{
	W3DMPO_GLUE(NullPrototypeClass)
public:
	NullPrototypeClass(void);
	NullPrototypeClass(const W3dNullObjectStruct &null);

	virtual const char *			Get_Name(void)	const			{ return Definition.Name; }
	virtual int								Get_Class_ID(void) const	{ return RenderObjClass::CLASSID_NULL; }
	virtual RenderObjClass *	Create(void)					{ return NEW_REF(Null3DObjClass,(Definition.Name)); }
	virtual void							DeleteSelf()						{ delete this; }

protected:
	W3dNullObjectStruct			Definition;
};


class NullLoaderClass : public PrototypeLoaderClass
{
public:
	virtual int						Chunk_Type(void) { return W3D_CHUNK_NULL_OBJECT; }
	virtual PrototypeClass *	Load_W3D(ChunkLoadClass & cload);
};


/*
** Instance of the default loader which the asset manager can
** automatically install at creation time
*/
extern NullLoaderClass _NullLoader;


#endif

