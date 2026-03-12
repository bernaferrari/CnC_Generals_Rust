// FIEL: W3DOverlordTruckDraw.cpp ////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, October 2002
// Desc: The Overlord has a super specific special need.  He needs his rider to draw explicitly after him,
// and he needs direct access to get that rider when everyone else can't see it because of the OverlordContain.
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/ContainModule.h"
#include "W3DDevice/GameClient/Module/W3DOverlordTruckDraw.h"

//-------------------------------------------------------------------------------------------------
W3DOverlordTruckDrawModuleData::W3DOverlordTruckDrawModuleData()
{
}

//-------------------------------------------------------------------------------------------------
W3DOverlordTruckDrawModuleData::~W3DOverlordTruckDrawModuleData()
{
}

//-------------------------------------------------------------------------------------------------
void W3DOverlordTruckDrawModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  W3DTruckDrawModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
W3DOverlordTruckDraw::W3DOverlordTruckDraw( Thing *thing, const ModuleData* moduleData )
: W3DTruckDraw( thing, moduleData )
{
} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
W3DOverlordTruckDraw::~W3DOverlordTruckDraw()
{
}

//-------------------------------------------------------------------------------------------------
void W3DOverlordTruckDraw::doDrawModule(const Matrix3D* transformMtx)
{
	W3DTruckDraw::doDrawModule(transformMtx);

	// Our big thing is that we get our specific passenger (the turret thing) and then wake it up and make it draw
	// It depends on us because our renderObject is only made correct in the act of drawing.
	Object *me = getDrawable()->getObject();
	if( me 
		&& me->getContain()
		&& me->getContain()->friend_getRider()
		&& me->getContain()->friend_getRider()->getDrawable()
		)
	{
		Drawable *riderDraw = me->getContain()->friend_getRider()->getDrawable();
		riderDraw->setColorTintEnvelope( *getDrawable()->getColorTintEnvelope() );

		riderDraw->notifyDrawableDependencyCleared();
		riderDraw->draw( NULL );// What the hell?  This param isn't used for anything
	}
}

//-------------------------------------------------------------------------------------------------
void W3DOverlordTruckDraw::setHidden(Bool h)
{
	W3DTruckDraw::setHidden(h);

	// We need to hide our rider, since he won't realize he's being contained in a contained container
	Object *me = getDrawable()->getObject();
	if( me 
		&& me->getContain()
		&& me->getContain()->friend_getRider()
		&& me->getContain()->friend_getRider()->getDrawable()
		)
	{
		me->getContain()->friend_getRider()->getDrawable()->setDrawableHidden(h);
	}
}
 
//-------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void W3DOverlordTruckDraw::crc( Xfer *xfer )
{

	// extend base class
	W3DTruckDraw::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void W3DOverlordTruckDraw::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	W3DTruckDraw::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void W3DOverlordTruckDraw::loadPostProcess( void )
{

	// extend base class
	W3DTruckDraw::loadPostProcess();

}  // end loadPostProcess
