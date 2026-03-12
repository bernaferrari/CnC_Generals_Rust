////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//  FILE: W3DOverlordAircraftDraw.h 
//  Author: Mark Lorenzen, April 2003
//  Desc: Units that recieve portable structure upgrades (like the Overlord Tank) have a super specific special need.
//  He needs his rider to draw explicitly after him,
//  and he needs direct access to get that rider when everyone else can't see it because of the OverlordContain.
//  In the case of aircraft (Helix, SpectreGunship, etc.) we need this draw module which mimics the OverlordTankDraw
//  but does not draw treads, trackmarks, turrets, etc. Whee!
//
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _W3D_OVERLORD_AIRCRAFT_DRAW_H_
#define _W3D_OVERLORD_AIRCRAFT_DRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "W3DDevice/GameClient/Module/W3DModelDraw.h"

//-------------------------------------------------------------------------------------------------
class W3DOverlordAircraftDrawModuleData : public W3DModelDrawModuleData
{
public:

	W3DOverlordAircraftDrawModuleData();
	~W3DOverlordAircraftDrawModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
class W3DOverlordAircraftDraw : public W3DModelDraw
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DOverlordAircraftDraw, "W3DOverlordAircraftDraw" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( W3DOverlordAircraftDraw, W3DOverlordAircraftDrawModuleData )
		
public:

	W3DOverlordAircraftDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

 	virtual void setHidden(Bool h);
	virtual void doDrawModule(const Matrix3D* transformMtx);

protected:

};

#endif

