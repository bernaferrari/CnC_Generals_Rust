// FILE: MapUtil.h /////////////////////////////////////////////////////////
// Author: Matt Campbell, December 2001
// Description: Map utility/convenience routines
////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __MAPUTIL_H__
#define __MAPUTIL_H__

#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"

#include "Common/STLTypedefs.h"

class GameWindow;
class AsciiString;
struct Coord3D;
struct FileInfo;
class Image;
class DataChunkInput;
struct DataChunkInfo;
// This matches the windows timestamp.
enum { SUPPLY_TECH_SIZE = 15};
typedef std::list <ICoord2D> ICoord2DList;

class TechAndSupplyImages
{
public:
	ICoord2DList m_techPosList;
	ICoord2DList m_supplyPosList;
};

struct WinTimeStamp
{
	UnsignedInt m_lowTimeStamp;
	UnsignedInt m_highTimeStamp;
};


class WaypointMap : public std::map<AsciiString, Coord3D>
{
public:
	void update( void );	///< returns the number of multiplayer start spots found
	Int m_numStartSpots;
};

typedef std::list <Coord3D> Coord3DList;

class MapMetaData
{
public:
	UnicodeString m_displayName;
	AsciiString m_nameLookupTag;
	Region3D m_extent;
	Int m_numPlayers;
	Bool m_isMultiplayer;

	Bool m_isOfficial;
	UnsignedInt m_filesize;
	UnsignedInt m_CRC;

	WinTimeStamp m_timestamp;

	WaypointMap m_waypoints;
	Coord3DList m_supplyPositions;
	Coord3DList m_techPositions;
	AsciiString m_fileName;
};

class MapCache : public std::map<AsciiString, MapMetaData>
{
public:
	MapCache() {}
	void updateCache( void );

	AsciiString getMapDir() const;
	AsciiString getUserMapDir() const;
	AsciiString getMapExtension() const;

	const MapMetaData *findMap(AsciiString mapName);

	// allow us to create a set of shippable maps to be in mapcache.ini.  For use with -buildMapCache.
	void addShippingMap(AsciiString mapName) { mapName.toLower(); m_allowedMaps.insert(mapName); }

private:
	Bool clearUnseenMaps( AsciiString dirName );
	void loadStandardMaps(void);
	Bool loadUserMaps(void);				// returns true if we needed to (re)parse a map
//	Bool addMap( AsciiString dirName, AsciiString fname, WinTimeStamp timestamp,
//		UnsignedInt filesize, Bool isOfficial );	///< returns true if it had to (re)parse the map
	Bool addMap( AsciiString dirName, AsciiString fname, FileInfo *fileInfo, Bool isOfficial); ///< returns true if it had to (re)parse the map
	void writeCacheINI( Bool userDir );

	static const char * m_mapCacheName;
	std::map<AsciiString, Bool> m_seen;

	std::set<AsciiString> m_allowedMaps;
};

extern MapCache *TheMapCache;
extern TechAndSupplyImages TheSupplyAndTechImageLocations;
Int populateMapListbox( GameWindow *listbox, Bool useSystemMaps, Bool isMultiplayer, AsciiString mapToSelect = AsciiString::TheEmptyString );		/// Read a list of maps from the run directory and fill in the listbox.  Return the selected index
Int populateMapListboxNoReset( GameWindow *listbox, Bool useSystemMaps, Bool isMultiplayer, AsciiString mapToSelect = AsciiString::TheEmptyString );		/// Read a list of maps from the run directory and fill in the listbox.  Return the selected index
Bool isValidMap( AsciiString mapName, Bool isMultiplayer );						/// Validate a map
Image *getMapPreviewImage( AsciiString mapName );
AsciiString getDefaultMap( Bool isMultiplayer );											/// Find a valid map
AsciiString getDefaultOfficialMap();
Bool isOfficialMap( AsciiString mapName );
Bool parseMapPreviewChunk(DataChunkInput &file, DataChunkInfo *info, void *userData);
void findDrawPositions( Int startX, Int startY, Int width, Int height, Region3D extent,
															 ICoord2D *ul, ICoord2D *lr );
Bool WouldMapTransfer( const AsciiString& mapName );
#endif // __MAPUTIL_H__
