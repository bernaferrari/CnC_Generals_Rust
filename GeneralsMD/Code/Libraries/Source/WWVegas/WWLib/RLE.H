/*********************************************************************************************** 
 ***              C O N F I D E N T I A L  ---  W E S T W O O D  S T U D I O S               *** 
 *********************************************************************************************** 
 *                                                                                             * 
 *                 Project Name : Command & Conquer                                            * 
 *                                                                                             * 
 *                     $Archive:: /Commando/Library/RLE.H                                     $* 
 *                                                                                             * 
 *                      $Author:: Greg_h                                                      $*
 *                                                                                             * 
 *                     $Modtime:: 7/22/97 11:37a                                              $*
 *                                                                                             * 
 *                    $Revision:: 1                                                           $*
 *                                                                                             *
 *---------------------------------------------------------------------------------------------* 
 * Functions:                                                                                  * 
 * - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */

#ifndef RLE_H
#define RLE_H

/*
**	This class will RLE compress and decompress arbitrary blocks of data. This RLE compression
**	is geared to compressing only runs of 0 bytes. This makes it useful for sprite encoding.
*/
class RLEEngine 
{
	public:
	
		/*
		**	Codec for arbitrary blocks.
		*/
		int Compress(void const * source, void * dest, int length) const;
		int Decompress(void const * source, void * dest, int length) const;

		/*
		**	Codec for length encoded blocks. This is useful for sprite storage.
		*/
		int Line_Compress(void const * source, void * dest, int length) const;
		int Line_Decompress(void const * source, void * dest) const;
};

#endif


