// FILE: W3DWaypointBuffer.h ////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
// Project:   Command & Conquers: Generals
//
// File name: W3DWaypointBuffer.h
//
// Created:   Kris Morness, October 2002
//
// Desc:      Draw buffer to handle all the waypoints in the scene. Waypoints
//            are rendered after terrain, after roads & bridges, and after
//            global fog, but before structures, objects, units, trees, etc.
//            This way if we have two waypoints at the bottom of a hill but
//            going through the hill, the line won't get cut off. However, 
//            structures and units on top of paths will render above it. Waypoints
//            are only shown for selected units while in waypoint plotting mode.
//
//-----------------------------------------------------------------------------

#pragma once

#ifndef __W3D_WAYPOINT_BUFFER_H
#define __W3D_WAYPOINT_BUFFER_H

//-----------------------------------------------------------------------------
//           Includes                                                      
//-----------------------------------------------------------------------------
#include "always.h"
#include "rendobj.h"
#include "w3d_file.h"
#include "dx8vertexbuffer.h"
#include "dx8indexbuffer.h"
#include "shader.h"
#include "vertmaterial.h"
#include "Lib/BaseType.h"
#include "common/GameType.h"

class SegmentedLineClass;

class W3DWaypointBuffer 
{	
	friend class HeightMapRenderObjClass;
public:

	W3DWaypointBuffer(void);
	~W3DWaypointBuffer(void);

	void drawWaypoints(RenderInfoClass &rinfo);
	void freeWaypointBuffers();


private:
  void setDefaultLineStyle();

	RenderObjClass *m_waypointNodeRobj;
	SegmentedLineClass *m_line;
	TextureClass *m_texture;
};

#endif  // end __W3D_WAYPOINT_BUFFER_H
