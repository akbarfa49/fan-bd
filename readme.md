prototype for black desert loot tracker

note
1. faster clear will need recording with more fps that lead to performance issue due to OCR taking tons of resource usage
2. implementing Bitmap/Template matching than using OCR can work but still need alot of effort and will reduce huge resource usage.
3. using npcap to capture the item via network is possible but need to mapping every single item with reversed HEX of item ID and some test if it will trigger EAC ban or not. if the reader interested the port for the game is :8889 also put in mind that the data is mixed with other this kinda make thing little bit more difficult, what you need is to find the correct hex pattern for loot/obtained item.

Tested fine for normal grinding with : 7800x3D RTX4060ti 10fps
