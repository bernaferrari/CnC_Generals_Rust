#if defined(_MSC_VER)
#pragma once
#endif

#ifndef SNAPPOINTS_H
#define SNAPPOINTS_H

#include "Max.h"

class ChunkSaveClass;
class INode;

/*
** This class simply contains static functions which will find
** helper points that should be exported with a w3d render object and
** export them in a chunk using the given ChunkSaveClass object.
*/
class SnapPointsClass
{
public:
	static void Export_Points(INode * scene_root,TimeValue time,ChunkSaveClass & csave);
};

#endif 
