#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000


#if defined(_MSC_VER)
#pragma once
#endif

#ifndef IOSTRUCT_H
#define IOSTRUCT_H

#ifndef BITTYPE_H
#include "bittype.h"
#endif

/*
** Some useful structures for writing/writing (safe from changes).
** The chunk IO classes contain code for reading and writing these.
*/
struct IOVector2Struct
{
	float32		X;
	float32		Y;
};

struct IOVector3Struct
{
	float32		X;							// X,Y,Z coordinates
	float32		Y;
	float32		Z;
};

struct IOVector4Struct
{
	float32		X;
	float32		Y;
	float32		Z;
	float32		W;
};

struct IOQuaternionStruct
{
	float32		Q[4];
};



#endif

