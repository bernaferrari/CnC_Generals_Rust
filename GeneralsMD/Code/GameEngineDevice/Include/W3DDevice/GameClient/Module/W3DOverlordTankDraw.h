// FIEL: W3DOverlordTankDraw.h ////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, October 2002
// Desc: The Overlord has a super specific special need.  He needs his rider to draw explicitly after him,
// and he needs direct access to get that rider when everyone else can't see it because of the OverlordContain.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _W3D_OVERLORD_TANK_DRAW_H_
#define _W3D_OVERLORD_TANK_DRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "W3DDevice/GameClient/Module/W3DTankDraw.h"

//-------------------------------------------------------------------------------------------------
class W3DOverlordTankDrawModuleData : public W3DTankDrawModuleData
{
public:
	AsciiString m_treadDebrisNameLeft;
	AsciiString m_treadDebrisNameRight;

	Real m_treadAnimationRate;	///<amount of tread texture to scroll per sec.  1.0 == full width.
	Real m_treadPivotSpeedFraction;	///<fraction of locomotor speed below which we allow pivoting.
	Real m_treadDriveSpeedFraction;	///<fraction of locomotor speed below which treads stop animating.

	W3DOverlordTankDrawModuleData();
	~W3DOverlordTankDrawModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
class W3DOverlordTankDraw : public W3DTankDraw
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DOverlordTankDraw, "W3DOverlordTankDraw" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( W3DOverlordTankDraw, W3DOverlordTankDrawModuleData )
		
public:

	W3DOverlordTankDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

 	virtual void setHidden(Bool h);
	virtual void doDrawModule(const Matrix3D* transformMtx);

protected:

};

#endif

