// FILE: W3DSupplyDraw.cpp ////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, September 2002
// Desc: Draw module reacts to SupplyStatus setting by hiding an equal number of the specified bone array.
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "W3DDevice/GameClient/Module/W3DSupplyDraw.h"

//-------------------------------------------------------------------------------------------------
W3DSupplyDrawModuleData::W3DSupplyDrawModuleData() 
{
}

//-------------------------------------------------------------------------------------------------
W3DSupplyDrawModuleData::~W3DSupplyDrawModuleData()
{
}

//-------------------------------------------------------------------------------------------------
void W3DSupplyDrawModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  W3DModelDrawModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "SupplyBonePrefix", INI::parseAsciiString, NULL, offsetof(W3DSupplyDrawModuleData, m_supplyBonePrefix) },

		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
W3DSupplyDraw::W3DSupplyDraw( Thing *thing, const ModuleData* moduleData ) : W3DModelDraw( thing, moduleData )
{	 
	m_totalBones = -1;
	m_lastNumberShown = 0;
}

//-------------------------------------------------------------------------------------------------
W3DSupplyDraw::~W3DSupplyDraw()
{
}

void W3DSupplyDraw::updateDrawModuleSupplyStatus( Int maxSupply, Int currentSupply )
{
	W3DModelDraw::updateDrawModuleSupplyStatus( maxSupply, currentSupply );

	AsciiString boneName = getW3DSupplyDrawModuleData()->m_supplyBonePrefix;
	if( m_totalBones == -1 )
	{
		m_totalBones = getDrawable()->getPristineBonePositions( boneName.str(), 1, NULL, NULL, INT_MAX );// The last arg is to guard the size of the arrays.  I am not passing any in, I am just counting bones.
		m_lastNumberShown = m_totalBones;
	}

	// Figure the % of our bones we should show, and if it is a different % than last time
	// start showing and hiding them.
	Int bonesToShow = ceil(m_totalBones * ( currentSupply / (float)maxSupply ));
	bonesToShow = min( bonesToShow, m_totalBones );

	if( bonesToShow != m_lastNumberShown )
	{
		// Show/hide the bones that are now different, the indices between last and now (low, high].
		Int lowIndex = min( m_lastNumberShown, bonesToShow );
		Int highIndex = max( m_lastNumberShown, bonesToShow );
		Bool hide = bonesToShow < m_lastNumberShown;
		Int currentIndex = lowIndex + 1;

		std::vector<ModelConditionInfo::HideShowSubObjInfo> boneVector;
		while( currentIndex <= highIndex )
		{
			char buffer[16];
			sprintf( buffer, "%s%02d", boneName.str(), currentIndex );
			ModelConditionInfo::HideShowSubObjInfo info;
			info.hide = hide;
			info.subObjName = buffer;
			boneVector.push_back(info);

			++currentIndex;
		}
		doHideShowSubObjs(&boneVector);

		m_lastNumberShown = bonesToShow;
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void W3DSupplyDraw::crc( Xfer *xfer )
{

	// extend base class
	W3DModelDraw::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void W3DSupplyDraw::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	W3DModelDraw::xfer( xfer );

	// Graham says there's no data to save here

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void W3DSupplyDraw::loadPostProcess( void )
{

	// extend base class
	W3DModelDraw::loadPostProcess();

}  // end loadPostProcess


