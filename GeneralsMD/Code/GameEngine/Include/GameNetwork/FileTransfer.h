///////////////////////////////////////////////////////////////////////////////////////
// FILE: FileTransfer.h
// Author: Matthew D. Campbell, December 2002
// Description: File Transfer wrapper using TheNetwork
///////////////////////////////////////////////////////////////////////////////////////

#pragma once
#ifndef __FILE_TRANSFER_H__
#define __FILE_TRANSFER_H__

class GameInfo;

// Convenience functions
AsciiString GetBasePathFromPath( AsciiString path );
AsciiString GetFileFromPath( AsciiString path );
AsciiString GetExtensionFromFile( AsciiString fname );
AsciiString GetBaseFileFromFile( AsciiString fname );
AsciiString GetPreviewFromMap( AsciiString path );
AsciiString GetINIFromMap( AsciiString path );
AsciiString GetStrFileFromMap( AsciiString path );
AsciiString GetSoloINIFromMap( AsciiString path );
AsciiString GetAssetUsageFromMap( AsciiString path );
AsciiString GetReadmeFromMap( AsciiString path );

// The meat of file (map) transfers
Bool DoAnyMapTransfers(GameInfo *game);

#endif // __FILE_TRANSFER_H__
