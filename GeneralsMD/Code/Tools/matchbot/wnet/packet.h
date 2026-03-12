/*************************************************************************** 
 *                                                                         * 
 *                 Project Name : Westwood Auto Registration App           * 
 *                                                                         * 
 *                    File Name : PACKET.H                                 * 
 *                                                                         * 
 *                   Programmer : Philip W. Gorrow                         * 
 *                                                                         * 
 *                   Start Date : 04/19/96                                 * 
 *                                                                         * 
 *                  Last Update : April 19, 1996 [PWG]                     * 
 *                                                                         * 
 * This header defines the functions for the PacketClass.  The packet      *
 * class is used to create a linked list of field entries which can be     * 
 * converted to a linear packet in a COMMS API compatible format.          *
 *                                                                         *
 * Packets can be created empty and then have fields added to them or can  *
 * be created from an existing linear packet.                              *
 *                                                                         *
 *-------------------------------------------------------------------------* 
 * Functions:                                                              * 
 * - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */

#include "field.h"
#include <wlib/wstypes.h>


class PacketClass
{
  public:

    PacketClass(short id = 0)
    {
      Size      = 0;
      ID        = id;
      Head      = 0;
    }
    PacketClass(char *cur_buf);
    ~PacketClass();

    //
    // This function allows us to add a field to the start of the list.  As the field is just
    //   a big linked list it makes no difference which end we add a member to.
    //
    void Add_Field(FieldClass *field);

    //
    // These conveniance functions allow us to add a field directly to the list without
    // having to worry about newing one first.
    //
    void Add_Field(char *field, char data) {Add_Field(new FieldClass(field, data));};
    void Add_Field(char *field, unsigned char data) {Add_Field(new FieldClass(field, data));};
    void Add_Field(char *field, short data) {Add_Field(new FieldClass(field, data));};
    void Add_Field(char *field, unsigned short data) {Add_Field(new FieldClass(field, data));};
    void Add_Field(char *field, long data) {Add_Field(new FieldClass(field, data));};
    void Add_Field(char *field, unsigned long data) {Add_Field(new FieldClass(field, data));};
    void Add_Field(char *field, char *data) {Add_Field(new FieldClass(field, data));};
    void Add_Field(char *field, void *data, int length) {Add_Field(new FieldClass(field, data, length));};

    //
    // These functions search for a field of a given name in the list and 
    // return the data via a reference value.
    //
    FieldClass *Find_Field(char *id);

    bit8 Get_Field(char *id, int &data);
    bit8 Get_Field(char *id, char &data);
    bit8 Get_Field(char *id, unsigned char &data);
    bit8 Get_Field(char *id, short &data);
    bit8 Get_Field(char *id, unsigned short &data);
    bit8 Get_Field(char *id, long &data);
    bit8 Get_Field(char *id, unsigned long &data);
    bit8 Get_Field(char *id, unsigned &data);
    bit8 Get_Field(char *id, char *data);
    bit8 Get_Field(char *id, void *data, int &length);
    unsigned short Get_Field_Size(char* id); 

    // gks 9/25/2000
    FieldClass *Get_Field_At(int position);
    int Get_Num_Fields();

    char *Create_Comms_Packet(int &size);
        
  private:
    unsigned short   Size;
    short            ID;
    FieldClass      *Head;
    FieldClass      *Current;
};

