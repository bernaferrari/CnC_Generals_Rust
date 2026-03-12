// FILE: W3DScienceModelDraw.cpp ////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, NOVEMBER 2002
// Desc: Draw module just like Model, except it only draws if the local player has the specified science
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "W3DDevice/GameClient/Module/W3DScienceModelDraw.h"

#include "Common/Player.h"
#include "Common/PlayerList.h"
#include "Common/Science.h"
#include "Common/Xfer.h"

//-------------------------------------------------------------------------------------------------
W3DScienceModelDrawModuleData::W3DScienceModelDrawModuleData() 
{
	m_requiredScience = SCIENCE_INVALID;
}

//-------------------------------------------------------------------------------------------------
W3DScienceModelDrawModuleData::~W3DScienceModelDrawModuleData()
{
}

//-------------------------------------------------------------------------------------------------
void W3DScienceModelDrawModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  W3DModelDrawModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "RequiredScience", INI::parseScience, NULL, offsetof(W3DScienceModelDrawModuleData, m_requiredScience) },

		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
W3DScienceModelDraw::W3DScienceModelDraw( Thing *thing, const ModuleData* moduleData ) : W3DModelDraw( thing, moduleData )
{	 
}

//-------------------------------------------------------------------------------------------------
W3DScienceModelDraw::~W3DScienceModelDraw()
{
}

//-------------------------------------------------------------------------------------------------
// All this does is stop the call path if we haven't been cleared to draw yet
void W3DScienceModelDraw::doDrawModule(const Matrix3D* transformMtx)
{
	ScienceType science = getW3DScienceModelDrawModuleData()->m_requiredScience;
	if( science == SCIENCE_INVALID )
	{
		DEBUG_ASSERTCRASH(science != SCIENCE_INVALID, ("ScienceModelDraw has invalid science as condition.") );
		setHidden( TRUE );
		return;
	}

	if( !ThePlayerList->getLocalPlayer()->hasScience(science) 
			&& ThePlayerList->getLocalPlayer()->isPlayerActive()
		)
	{
		// We just don't draw for people without our science except for Observers
		setHidden( TRUE );
		return;
	}

	W3DModelDraw::doDrawModule(transformMtx);
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void W3DScienceModelDraw::crc( Xfer *xfer )
{

	// extend base class
	W3DModelDraw::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void W3DScienceModelDraw::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	W3DModelDraw::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void W3DScienceModelDraw::loadPostProcess( void )
{

	// extend base class
	W3DModelDraw::loadPostProcess();

}  // end loadPostProcess


