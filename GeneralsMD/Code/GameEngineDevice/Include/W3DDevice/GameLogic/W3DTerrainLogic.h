// FILE: W3DTerrainLogic.h ////////////////////////////////////////////////////////////////////////
// W3D implementation details for logical terrain
// Author: Colin Day, April 2001
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DTERRAINLOGIC_H_
#define __W3DTERRAINLOGIC_H_

#include "GameLogic/TerrainLogic.h"

//-------------------------------------------------------------------------------------------------
/** W3D specific implementation details for logical terrain ... we have this
  * because the logic and visual terrain are closely tied together in that
	* they represent the same thing, but need to be broken up into logical
	* and graphical representations */
//-------------------------------------------------------------------------------------------------
class W3DTerrainLogic : public TerrainLogic
{

public:

	W3DTerrainLogic();
	virtual ~W3DTerrainLogic();

	virtual void init( void );		///< Init
	virtual void reset( void );		///< Reset
	virtual void update( void );	///< Update

	/// @todo The loading of the raw height data should be device independent
	virtual Bool loadMap( AsciiString filename , Bool query );
	virtual void newMap( Bool saveGame );	///< Initialize the logic for new map.

	virtual Real getGroundHeight( Real x, Real y, Coord3D* normal = NULL ) const;

	virtual Bool isCliffCell( Real x, Real y) const;			///< is point cliff cell.

	virtual Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D* normal = NULL, Bool clip = true) const;

	virtual void getExtent( Region3D *extent ) const ;					///< Get the 3D extent of the terrain in world coordinates

	virtual void getMaximumPathfindExtent( Region3D *extent ) const;

	virtual void getExtentIncludingBorder( Region3D *extent ) const;

	virtual Bool isClearLineOfSight(const Coord3D& pos, const Coord3D& posOther) const;

protected:

	// snapshot methods
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

	Real m_mapMinZ;	///< Minimum terrain z value.
	Real m_mapMaxZ;	///< Maximum terrain z value.

};  // end W3DTerrainLogic

#endif  // end __W3DTERRAINLOGIC_H_
